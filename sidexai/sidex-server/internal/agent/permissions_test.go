package agent

import "testing"

func TestCheckPermission(t *testing.T) {
	cases := []struct {
		tool string
		mode PermissionMode
		want PermissionDecision
	}{
		// Tools that start processes or reach outside the agent always ask.
		{"mcp_connect", PermissionDefault, PermAsk},
		{"mcp_connect", PermissionAutoEdits, PermAsk},
		{"mcp_call_tool", PermissionDefault, PermAsk},
		{"rollback", PermissionAutoEdits, PermAsk},
		{"shell", PermissionDefault, PermAsk},
		{"shell", PermissionAutoEdits, PermAsk},
		// Unclassified names, e.g. tools provided by an MCP server, ask.
		{"some_mcp_server_tool", PermissionDefault, PermAsk},
		{"some_mcp_server_tool", PermissionAutoEdits, PermAsk},
		// File edits ask by default and are approved in auto_edits.
		{"write_file", PermissionDefault, PermAsk},
		{"write_file", PermissionAutoEdits, PermAllow},
		// Read-only and safe meta tools never ask.
		{"read_file", PermissionDefault, PermAllow},
		{"plan_create", PermissionDefault, PermAllow},
		{"checkpoint", PermissionAutoEdits, PermAllow},
		{"mcp_list_tools", PermissionDefault, PermAllow},
		// Plan mode allows only read-only tools.
		{"read_file", PermissionPlanOnly, PermAllow},
		{"mcp_connect", PermissionPlanOnly, PermDeny},
		{"some_mcp_server_tool", PermissionPlanOnly, PermDeny},
		// auto_all approves everything, as the user chose.
		{"mcp_connect", PermissionAutoAll, PermAllow},
	}
	for _, c := range cases {
		if got := CheckPermission(c.tool, c.mode); got != c.want {
			t.Errorf("CheckPermission(%q, %s) = %d, want %d",
				c.tool, PermissionModeString(c.mode), got, c.want)
		}
	}
}
