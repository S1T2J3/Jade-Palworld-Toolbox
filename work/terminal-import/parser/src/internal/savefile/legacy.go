// SPDX-License-Identifier: GPL-3.0-or-later
// Pal Atlas additions, 2026. Compatibility with legacy zlib and Xbox headers.
package savefile

import (
	"bytes"
	"compress/zlib"
	"encoding/binary"
	"fmt"
	"io"
)

func inflate(data []byte, expected uint32) ([]byte, error) {
	r, err := zlib.NewReader(bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	defer r.Close()
	out := make([]byte, int(expected))
	if _, err = io.ReadFull(r, out); err != nil {
		return nil, err
	}
	// Force the reader to check the Adler checksum and ensure exact length.
	var tail [1]byte
	if n, err := r.Read(tail[:]); n != 0 || err != io.EOF {
		return nil, fmt.Errorf("savefile: zlib length or checksum mismatch")
	}
	return out, nil
}

func decodeLegacy(data []byte, limits Limits) ([]byte, ContainerHeader, error) {
	h := ContainerHeader{RawSize: binary.LittleEndian.Uint32(data), CompressedSize: binary.LittleEndian.Uint32(data[4:]), Magic: "PlZ", SaveType: data[11]}
	if h.RawSize == 0 || int64(h.RawSize) > limits.MaxArchiveBytes || h.CompressedSize == 0 || int64(h.CompressedSize) > limits.MaxFileBytes {
		return nil, h, fmt.Errorf("savefile: invalid zlib archive size")
	}
	compressed := data[12:]
	switch h.SaveType {
	case '1':
		if len(compressed) != int(h.CompressedSize) {
			return nil, h, fmt.Errorf("savefile: zlib compressed size mismatch")
		}
	case '2':
		var err error
		compressed, err = inflate(compressed, h.CompressedSize)
		if err != nil {
			return nil, h, err
		}
	default:
		return nil, h, fmt.Errorf("savefile: unknown zlib compression type %#x", h.SaveType)
	}
	raw, err := inflate(compressed, h.RawSize)
	if err == nil && !bytes.HasPrefix(raw, []byte("GVAS")) {
		err = fmt.Errorf("savefile: zlib payload is not GVAS")
	}
	return raw, h, err
}
