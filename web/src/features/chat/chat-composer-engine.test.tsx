import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { EngineStatusResponse } from "../../api/contracts";
import { ChatComposer } from "./chat-composer";

const queryState = vi.hoisted(() => ({
  engineStatus: {
    engine: "native",
    label: "Native",
    external: false,
    unavailable_features: []
  } as EngineStatusResponse
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: ({ queryKey }: { queryKey?: readonly unknown[] }) => ({
    data: queryKey?.[0] === "engine-status" ? queryState.engineStatus : null,
    isLoading: false
  }),
  useQueryClient: () => ({ invalidateQueries: () => Promise.resolve() }),
  useMutation: () => ({ mutate: () => {}, isPending: false })
}));

vi.mock("./model-thinking-selector", () => ({
  ModelThinkingSelector: ({ selection }: { selection: { model: string } | null }) => (
    <span data-testid="native-model-selector">{selection?.model}</span>
  )
}));
vi.mock("./composer/attachment-strip", () => ({ AttachmentStrip: () => null }));
vi.mock("./composer/composer-textarea", () => ({ ComposerTextarea: () => null }));
vi.mock("./agent-selector", () => ({ AgentSelector: () => null }));
vi.mock("../workspaces/workspace-switcher", () => ({ WorkspaceSwitcher: () => null }));
vi.mock("../usage/system-usage", () => ({
  SystemUsage: () => <span data-testid="native-system-usage" />
}));
vi.mock("../goals/goal-control", () => ({ GoalControl: () => null }));
vi.mock("../runtime-activity/use-runtime-activity", () => ({
  useRuntimeActivity: () => ({ runningTasks: 0, runningSubagents: 0 })
}));

/** 渲染带固定内置模型选择的输入区。 */
function renderComposer(): string {
  return renderToStaticMarkup(
    <ChatComposer
      value=""
      mode="yolo"
      attachments={[]}
      historyEntries={[]}
      thinkingLevel="auto"
      choices={[]}
      selection={{ providerId: "openai", providerName: "OpenAI", model: "gpt-native" }}
      pendingSelection={null}
      modelLoading={false}
      running={false}
      runStatus="idle"
      sessionAvailable
      undoAvailable={false}
      agentChoices={[]}
      agentSelection={null}
      agentLoading={false}
      onChange={() => undefined}
      onModeChange={() => undefined}
      onThinkingLevelChange={() => undefined}
      onAddImages={async () => undefined}
      onRemoveAttachment={() => undefined}
      onModelSelect={() => undefined}
      onSubmit={() => undefined}
      onStop={() => undefined}
      onUndo={() => undefined}
      onAgentSelect={() => undefined}
      onCompact={async () => undefined}
      onContinueGoal={async () => undefined}
    />
  );
}

describe("ChatComposer engine model display", () => {
  beforeEach(() => {
    queryState.engineStatus = {
      engine: "native",
      label: "Native",
      external: false,
      unavailable_features: []
    };
  });

  it("shows the native model selector for the native engine", () => {
    const html = renderComposer();

    expect(html).toContain("native-model-selector");
    expect(html).toContain("native-system-usage");
  });

  it("offers a connect entry for the Claude external engine before handshake", () => {
    queryState.engineStatus = {
      engine: "claude_code",
      label: "Claude Code",
      external: true,
      unavailable_features: ["context compaction"]
    };

    const html = renderComposer();

    // 未握手时徽标是连接入口，尚不展示品牌图标
    expect(html).toContain("Claude Code");
    expect(html).toContain("engine-connection-badge is-idle");
    expect(html).not.toContain("native-model-selector");
    expect(html).not.toContain("native-system-usage");
    expect(html).not.toContain("gpt-native");
  });

  it("marks the badge as connected once the engine reports runtime state", () => {
    queryState.engineStatus = {
      engine: "claude_code",
      label: "Claude Code",
      external: true,
      unavailable_features: ["context compaction"],
      acp_runtime: {
        connected: true,
        agent_name: "claude-code",
        agent_version: "1.0.0",
        capabilities: undefined,
        auth_methods: null,
        config_options: [],
        modes: null,
        available_commands: null,
        native_equivalents: null
      }
    };

    const html = renderComposer();

    expect(html).toContain("engine-connection-badge is-connected");
    expect(html).toContain('data-agent-engine-brand="claude-code"');
  });
});
