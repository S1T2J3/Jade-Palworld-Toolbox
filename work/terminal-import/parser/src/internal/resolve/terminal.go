// SPDX-License-Identifier: GPL-3.0-or-later
// Pal Atlas additions, 2026. Read only the ordinary Palbox of each player.
package resolve

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/LukeHollandDev/palworld-save-reader/internal/gvas"
	"github.com/LukeHollandDev/palworld-save-reader/internal/palworld"
	"github.com/LukeHollandDev/palworld-save-reader/internal/savefile"
)

type TerminalPlayerFile struct {
	ID   string
	Path string
}
type TerminalOwner struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}
type TerminalRecord struct {
	InstanceID string   `json:"instanceId"`
	SpeciesKey string   `json:"speciesKey"`
	Name       string   `json:"name"`
	Sex        string   `json:"sex"`
	Traits     []string `json:"traits"`
	OwnerID    string   `json:"ownerId"`
	slot       int32
}
type TerminalResult struct {
	Owners   []TerminalOwner  `json:"owners"`
	Records  []TerminalRecord `json:"records"`
	Warnings []string         `json:"warnings"`
}
type placement struct {
	owner string
	slot  int32
}

func canonical(id gvas.GUID) string { return strings.ToLower(strings.ReplaceAll(id.String(), "-", "")) }

// Multi-blob WGS world files are presented in numeric order by the host. Do
// not write them to a temporary world, or deserialize unrelated game data to JSON.
func openParts(paths []string, maxBytes int64) (*savefile.Save, error) {
	if len(paths) == 0 || len(paths) > 512 {
		return nil, fmt.Errorf("存档缺少文件或分块数量异常")
	}
	collected := bytes.Buffer{}
	for part, path := range paths {
		if !filepath.IsAbs(path) {
			return nil, fmt.Errorf("存档路径必须是绝对路径")
		}
		f, err := os.Open(path)
		if err != nil {
			return nil, err
		}
		info, err := f.Stat()
		capacity := maxBytes - int64(collected.Len())
		if err != nil || !info.Mode().IsRegular() || info.Size() <= 0 || info.Size() > capacity {
			f.Close()
			return nil, fmt.Errorf("存档文件大小异常或正在同步")
		}
		size := info.Size()
		if part > 0 {
			// XGP writes another 12-byte CNK wrapper on continuation
			// files, followed immediately by bytes from the same stream.
			var prefix [24]byte
			n, _ := f.ReadAt(prefix[:], 0)
			signature := string(prefix[8:12])
			if signature == "PlM1" || signature == "PlZ1" || signature == "PlZ2" || string(prefix[:4]) == "GVAS" {
				f.Close()
				return nil, fmt.Errorf("分块清单包含多个完整世界存档")
			}
			if n >= 12 && string(prefix[8:11]) == "CNK" {
				nested := string(prefix[20:24])
				if n == 24 && (nested == "PlM1" || nested == "PlZ1" || nested == "PlZ2") {
					f.Close()
					return nil, fmt.Errorf("分块清单包含多个完整世界存档")
				}
				if _, err = f.Seek(12, io.SeekStart); err != nil {
					f.Close()
					return nil, err
				}
				size -= 12
			}
		}
		collected.Grow(int(size))
		n, err := io.CopyN(&collected, f, size)
		var tail [1]byte
		more, eof := f.Read(tail[:])
		f.Close()
		if err != nil || n != size || more != 0 || eof != io.EOF {
			return nil, fmt.Errorf("存档大小在读取时发生变化")
		}
	}
	return savefile.Read(collected.Bytes(), savefile.Options{Limits: savefile.Limits{MaxFileBytes: maxBytes, MaxArchiveBytes: maxBytes}, GVAS: gvas.Options{TypeHints: palworld.TypeHints()}})
}

func ReadTerminals(level []string, players []TerminalPlayerFile) (*TerminalResult, error) {
	if len(players) == 0 {
		return nil, fmt.Errorf("缺少 Players 玩家存档，无法确定帕鲁终端的归属。")
	}
	result := &TerminalResult{Owners: []TerminalOwner{}, Records: []TerminalRecord{}, Warnings: []string{}}
	owners := map[gvas.GUID]int{}
	boxes := map[gvas.GUID]string{}
	skipped := 0
	for _, p := range players {
		player, err := openParts([]string{p.Path}, 64<<20)
		if err != nil {
			skipped++
			continue
		}
		uid, ok := guid(player.Properties, playerUIDPath)
		if !ok || uid.IsZero() {
			skipped++
			continue
		}
		if p.ID != "" {
			fileUID, parseErr := gvas.ParseGUID(p.ID)
			if parseErr != nil || fileUID != uid {
				skipped++
				continue
			}
		}
		if _, exists := owners[uid]; exists {
			return nil, fmt.Errorf("多个玩家文件使用同一个角色编号，请等待游戏完成保存。")
		}
		id := canonical(uid)
		owners[uid] = len(result.Owners)
		result.Owners = append(result.Owners, TerminalOwner{ID: id})
		box, ok := guid(player.Properties, playerStoragePath)
		if !ok || box.IsZero() {
			result.Warnings = append(result.Warnings, "一位角色的玩家存档中没有帕鲁终端编号。")
			continue
		}
		if prior := boxes[box]; prior != "" && prior != id {
			return nil, fmt.Errorf("同一个终端出现两个角色归属，暂时无法导入。")
		}
		boxes[box] = id
	}
	if len(owners) == 0 {
		return nil, fmt.Errorf("缺少可读取的角色资料，请检查同一次保存的 Players 与世界文件。")
	}
	if skipped > 0 {
		result.Warnings = append(result.Warnings, fmt.Sprintf("有 %d 份玩家资料暂时无法读取。", skipped))
	}

	saved, err := openParts(level, 1<<30)
	if err != nil {
		return nil, fmt.Errorf("无法读取世界存档：%w", err)
	}
	world, ok := value[gvas.Properties](saved.Properties, "worldSaveData")
	if !ok {
		return nil, fmt.Errorf("所选文件中缺少世界数据。")
	}
	slots := map[gvas.GUID]placement{}
	if err := readTerminalSlots(world, boxes, slots, result); err != nil {
		return nil, err
	}
	if err := readTerminalPals(world, owners, slots, result); err != nil {
		return nil, err
	}
	sort.SliceStable(result.Records, func(i, j int) bool {
		a, b := result.Records[i], result.Records[j]
		if a.OwnerID != b.OwnerID {
			return a.OwnerID < b.OwnerID
		}
		if a.slot != b.slot {
			return a.slot < b.slot
		}
		return a.InstanceID < b.InstanceID
	})
	return result, nil
}

func readTerminalSlots(world gvas.Properties, boxes map[gvas.GUID]string, wanted map[gvas.GUID]placement, result *TerminalResult) error {
	containers, ok := value[*gvas.MapValue](world, "CharacterContainerSaveData")
	if !ok {
		return fmt.Errorf("世界存档中缺少帕鲁容器数据。")
	}
	found := map[gvas.GUID]bool{}
	bad := 0
	it := containers.Iterator()
	for it.Next() {
		key, _ := it.Entry().Key.(gvas.Properties)
		id, _ := guid(key, "ID")
		owner, match := boxes[id]
		if !match {
			continue
		}
		found[id] = true
		entry, _ := it.Entry().Value.(gvas.Properties)
		slots, ok := structArray(entry, "Slots")
		if !ok {
			if entry.Find("Slots") != nil {
				bad++
			}
			continue
		}
		elements := slots.Iterator()
		for elements.Next() {
			slot, _ := elements.Value().(gvas.Properties)
			// Legacy slots can carry an all-zero tagged IndividualId next
			// to a populated RawData reference. Read the live reference.
			instance, _ := guid(slot, "IndividualId.InstanceId")
			if data, hasRaw := blob(slot, "RawData"); hasRaw && len(data) > 0 {
				raw, err := palworld.DecodeCharacterSlot(data)
				if err != nil {
					bad++
					continue
				}
				if !raw.InstanceID.IsZero() {
					instance = raw.InstanceID
				}
			}
			if instance.IsZero() {
				continue
			}
			if prior, exists := wanted[instance]; exists && prior.owner != owner {
				return fmt.Errorf("同一只帕鲁同时出现在不同玩家的终端，暂时无法确定归属。")
			}
			index, _ := value[int32](slot, "SlotIndex")
			wanted[instance] = placement{owner, index}
		}
		if err := elements.Err(); err != nil {
			return fmt.Errorf("无法读取终端格位：%w", err)
		}
	}
	if err := it.Err(); err != nil {
		return fmt.Errorf("无法读取帕鲁容器：%w", err)
	}
	if len(found) < len(boxes) {
		result.Warnings = append(result.Warnings, fmt.Sprintf("有 %d 个玩家终端尚未在世界文件中找到。", len(boxes)-len(found)))
	}
	if bad > 0 {
		result.Warnings = append(result.Warnings, fmt.Sprintf("有 %d 个终端格位未能辨认。", bad))
	}
	return nil
}

func readTerminalPals(world gvas.Properties, owners map[gvas.GUID]int, slots map[gvas.GUID]placement, result *TerminalResult) error {
	characters, ok := value[*gvas.MapValue](world, "CharacterSaveParameterMap")
	if !ok {
		return fmt.Errorf("世界存档中缺少帕鲁个体数据。")
	}
	it := characters.Iterator()
	for it.Next() {
		key, _ := it.Entry().Key.(gvas.Properties)
		uid, _ := guid(key, "PlayerUId")
		instance, _ := guid(key, "InstanceId")
		ownerIndex, isOwner := owners[uid]
		at, isPal := slots[instance]
		if !isOwner && !isPal {
			continue
		}
		entry, _ := it.Entry().Value.(gvas.Properties)
		data, ok := blob(entry, "RawData")
		if !ok {
			continue
		}
		character, err := palworld.DecodeCharacter(data)
		if err != nil {
			continue
		}
		p := character.Parameters()
		if p == nil {
			continue
		}
		nickname, _ := value[string](p, recordNickname)
		if isPlayer, _ := value[bool](p, recordIsPlayer); isPlayer {
			if isOwner {
				result.Owners[ownerIndex].Name = nickname
			}
			continue
		}
		if !isPal {
			continue
		}
		species, _ := value[string](p, recordSpecies)
		if species == "" {
			continue
		}
		sex := "unknown"
		gender, _ := enumName(p, recordGender)
		switch strings.ToLower(gender) {
		case "male":
			sex = "M"
		case "female":
			sex = "F"
		}
		traits := names(p, recordPassives)
		if traits == nil {
			traits = []string{}
		}
		result.Records = append(result.Records, TerminalRecord{canonical(instance), species, nickname, sex, traits, at.owner, at.slot})
		delete(slots, instance)
	}
	if err := it.Err(); err != nil {
		return fmt.Errorf("无法读取帕鲁个体：%w", err)
	}
	if len(slots) > 0 {
		result.Warnings = append(result.Warnings, fmt.Sprintf("有 %d 个终端格位缺少可读取的个体资料。可在游戏保存完成后重试。", len(slots)))
	}
	return nil
}
