// SPDX-License-Identifier: GPL-3.0-or-later
package main

import (
	"encoding/json"
	"fmt"
	"github.com/LukeHollandDev/palworld-save-reader/internal/resolve"
	"io"
	"os"
)

func run() error {
	var in struct {
		Level   []string
		Players []resolve.TerminalPlayerFile
	}
	data, err := io.ReadAll(io.LimitReader(os.Stdin, (1<<21)+1))
	if err != nil || len(data) > 1<<21 {
		return fmt.Errorf("存档文件清单过大或无法读取。")
	}
	if err = json.Unmarshal(data, &in); err != nil {
		return fmt.Errorf("存档文件清单无效。")
	}
	result, err := resolve.ReadTerminals(in.Level, in.Players)
	if err != nil {
		return err
	}
	return json.NewEncoder(os.Stdout).Encode(result)
}

func main() {
	if err := run(); err != nil {
		json.NewEncoder(os.Stdout).Encode(map[string]string{"error": err.Error()})
		os.Exit(1)
	}
}
