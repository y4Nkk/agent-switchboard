import { actionProps } from "../content/site-content";
import { useSitePreferences } from "../use-site-preferences";
import { GithubIcon, StarIcon } from "./icons";

export function FinalCta() {
  const { content } = useSitePreferences();
  const actions = content.final.actions;
  return (
    <section className="final">
      <div className="site-container">
        <h2>{content.final.title}</h2>
        <div className="final-actions">
          {actions.map((action) => {
            const Icon = action.icon === "github" ? GithubIcon : action.icon === "star" ? StarIcon : null;
            return (
              <a
                key={action.label}
                className={`site-btn site-btn-${action.variant}`}
                {...actionProps(action)}
              >
                {Icon ? <Icon size={15} /> : null}
                {action.label}
              </a>
            );
          })}
        </div>
        <footer className="site-footer">{content.footer.note}</footer>
      </div>
    </section>
  );
}
