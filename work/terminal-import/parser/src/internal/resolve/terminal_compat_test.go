// SPDX-License-Identifier: GPL-3.0-or-later
package resolve

import "testing"

func TestTerminalImportExcludesOtherContainers(t *testing.T) {
	r := openTestResolver(t)
	var files []TerminalPlayerFile
	for _, path := range r.set.Players {
		files = append(files, TerminalPlayerFile{Path: path})
	}
	result, err := ReadTerminals([]string{r.set.Level}, files)
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Records) != 2 || len(result.Owners) != 2 {
		t.Fatalf("unexpected terminal result: %+v", result)
	}
	if result.Records[0].InstanceID != canonical(id(204)) || result.Records[0].Name != "Sparky" || result.Records[1].InstanceID != canonical(id(203)) {
		t.Fatalf("wrong terminal slots: %+v", result.Records)
	}
	for _, p := range result.Records {
		if p.OwnerID != canonical(id(oneUID)) {
			t.Fatalf("wrong owner: %+v", p)
		}
	}
}
