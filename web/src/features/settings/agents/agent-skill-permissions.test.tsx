import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { AgentSkillPermissions, updateSkillPermission } from "./agent-skill-permissions";

const skills = [
  { name: "code-review", description: "检查代码变更" },
  { name: "research", description: "检索并整理资料" }
];

describe("AgentSkillPermissions", () => {
  it("渲染搜索、状态筛选、批量操作和每行三态选择", () => {
    const html = renderToStaticMarkup(
      <AgentSkillPermissions
        skills={skills}
        fullNames={["code-review"]}
        namedNames={[]}
        onChange={vi.fn()}
      />
    );

    expect(html).toContain('placeholder="搜索 Skill"');
    expect(html).toContain('aria-label="筛选 Skill 状态"');
    expect(html).toContain('aria-label="设置 code-review 的权限"');
    expect(html).toContain("全部启用");
    expect(html).toContain("全部关闭");
    expect(html).not.toContain("draggable");
  });

  it("切换三态权限时输出互斥的 fullNames 和 namedNames", () => {
    expect(updateSkillPermission(["code-review"], [], "research", "named"))
      .toEqual({ fullNames: ["code-review"], namedNames: ["research"] });
    expect(updateSkillPermission(["code-review"], ["research"], "code-review", "off"))
      .toEqual({ fullNames: [], namedNames: ["research"] });
    expect(updateSkillPermission([], ["research"], "research", "full"))
      .toEqual({ fullNames: ["research"], namedNames: [] });
  });
});
