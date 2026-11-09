// SPDX-License-Identifier: GPL-3.0-or-later
package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/LukeHollandDev/palworld-save-reader/internal/resolve"
)

const maxInput = 2 << 20
const maxOutput = 64 << 20

type boundedOutput struct {
	target    io.Writer
	remaining int
}

func (out *boundedOutput) Write(data []byte) (int, error) {
	if len(data) > out.remaining {
		return 0, fmt.Errorf("终端数据量超出本次读取范围。")
	}
	n, err := out.target.Write(data)
	out.remaining -= n
	return n, err
}

func read(input io.Reader, output io.Writer) error {
	var in struct {
		Level   []string
		Players []resolve.TerminalPlayerFile
	}
	data, err := io.ReadAll(io.LimitReader(input, maxInput+1))
	if err != nil || len(data) > maxInput {
		return fmt.Errorf("存档文件清单过大或无法读取。")
	}
	if err = json.Unmarshal(data, &in); err != nil {
		return fmt.Errorf("存档文件清单无效。")
	}
	result, err := resolve.ReadTerminals(in.Level, in.Players)
	if err != nil {
		return err
	}
	return json.NewEncoder(&boundedOutput{output, maxOutput}).Encode(result)
}

func respond(input io.Reader, output io.Writer) int {
	if err := read(input, output); err != nil {
		_ = json.NewEncoder(output).Encode(map[string]string{"error": err.Error()})
		return 1
	}
	return 0
}

func validID(value string) bool {
	if len(value) != 32 || value != strings.ToLower(value) {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func run(args []string) int {
	if len(args) == 0 {
		return respond(os.Stdin, os.Stdout)
	}
	if len(args) != 4 || args[0] != "--terminal-ipc" || !filepath.IsAbs(args[1]) ||
		!validID(args[2]) || !validID(args[3]) || args[2] == args[3] {
		return 2
	}
	ipc := filepath.Join(args[1], "ipc")
	request := filepath.Join(ipc, "read-"+args[2]+".json")
	input, err := os.Open(request)
	if err != nil {
		return 1
	}
	defer input.Close()
	output, err := os.OpenFile(filepath.Join(ipc, "read-"+args[3]+".json"), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return 1
	}
	status := respond(input, output)
	if err := output.Close(); err != nil {
		return 1
	}
	return status
}

func main() {
	os.Exit(run(os.Args[1:]))
}
