import type { SessionProject } from "../../api/client";
import { useI18n } from "../../i18n";
import { clientFullName } from "../../lib/client-name";
import { Select } from "../Select";
import type { SessionSearch } from "./use-session-search";

function projectKey(project: Pick<SessionProject, "app" | "projectDir">): string {
  return JSON.stringify([project.app, project.projectDir]);
}

function projectName(dir: string | null, fallback: string): string {
  if (!dir) return fallback;
  const parts = dir.replace(/[\\/]+$/u, "").split(/[\\/]/u).filter(Boolean);
  return parts.at(-1) ?? dir;
}

export function SessionProjects({ search, disabled }: { search: SessionSearch; disabled: boolean }) {
  const { t } = useI18n();
  const total = search.projects.reduce((sum, project) => sum + project.count, 0);
  const selected = search.project;
  const projects = selected && !search.projects.some((project) => project.app === selected.app
    && project.projectDir === selected.projectDir) ? [...search.projects, { ...selected, count: 0 }] : search.projects;
  const fallback = t("sessions.projects.none");
  const names = projects.map((project) => projectName(project.projectDir, fallback));
  const repeated = new Map<string, number>();
  projects.forEach((project, index) => {
    const key = `${project.app}:${names[index]}`;
    repeated.set(key, (repeated.get(key) ?? 0) + 1);
  });
  const options = [{ value: "all", label: `${t("sessions.projects.all")} · ${total}` },
    ...projects.map((project, index) => {
      const parts = project.projectDir?.replace(/[\\/]+$/u, "").split(/[\\/]/u).filter(Boolean) ?? [];
      const name = (repeated.get(`${project.app}:${names[index]}`) ?? 0) > 1 && parts.length > 1
        ? `${parts.at(-2)}\\${names[index]}` : names[index];
      return { value: projectKey(project), label: `${name} · ${clientFullName(project.app)} · ${project.count}`,
        tooltip: `${clientFullName(project.app)} · ${project.projectDir ?? fallback} · ${project.count}` };
    })];
  const tooltip = selected ? `${clientFullName(selected.app)} · ${selected.projectDir ?? fallback}`
    : t("sessions.projects.all");
  return <div className="asb-session-project-filter" aria-busy={search.busy}>
    <Select value={selected ? projectKey(selected) : "all"} options={options}
      ariaLabel={t("sessions.projects.heading")} disabled={disabled} triggerTooltip={tooltip}
      contentClassName="asb-session-project-menu" onChange={(value) => {
        if (value === "all") { search.changeProject(null); return; }
        const project = projects.find((item) => projectKey(item) === value);
        if (project) search.changeProject({ app: project.app, projectDir: project.projectDir });
      }} />
  </div>;
}
