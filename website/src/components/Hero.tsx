import { actionProps } from "../content/site-content";
import { useSitePreferences } from "../use-site-preferences";
import { GithubIcon } from "./icons";

function AppReplica() {
  const { content } = useSitePreferences();
  const { routeCardLabel, routeFieldLabels } = content.hero;
  return (
    <div className="app-replica" aria-label={content.hero.title}>
      <div className="app-replica-bar">
        <span className="app-replica-brand">
          <img src="/favicon.svg" alt="" />
          {content.brand}
        </span>
        <nav className="app-replica-nav" aria-hidden="true">
          {content.hero.appShell.nav.map((item, index) => (
            <span key={item} data-active={index === 0 || undefined}>
              {item}
            </span>
          ))}
        </nav>
      </div>
      <div className="app-replica-body">
        <section className="app-replica-panel">
          <div className="app-replica-panel-head">
            <span className="app-replica-panel-title">
              {content.hero.appShell.enabledPanel}
            </span>
          </div>
          <div className="route-stack">
            {content.hero.cards.map((card) => (
              <article
                key={card.client}
                className={`route-card route-card-${card.tone}`}
                aria-label={routeCardLabel(card.client)}
              >
                <span className="route-card-client">{card.client}</span>
                <p className="route-card-provider">{card.provider}</p>
                <dl className="route-card-fields">
                  <div>
                    <dt>{routeFieldLabels.model}</dt>
                    <dd>{card.model}</dd>
                  </div>
                  <div>
                    <dt>{routeFieldLabels.endpoint}</dt>
                    <dd>{card.endpoint}</dd>
                  </div>
                  <div>
                    <dt>{routeFieldLabels.access}</dt>
                    <dd>{card.access}</dd>
                  </div>
                </dl>
              </article>
            ))}
          </div>
        </section>
        <section className="app-replica-panel">
          <div className="app-replica-panel-head">
            <span className="app-replica-panel-title">
              {content.hero.appShell.statusPanel}
            </span>
            <span className="app-replica-panel-action" aria-hidden="true">
              {content.hero.appShell.statusAction}
            </span>
          </div>
          <div className="status-grid">
            {content.hero.status.map((card) => (
              <article className="status-card" key={card.client}>
                <div className="status-card-head">
                  <strong>{card.client}</strong>
                  <span className="status-pill">{card.status}</span>
                </div>
                <dl>
                  {card.rows.map(([label, value]) => (
                    <div className="status-row" key={label}>
                      <dt>{label}</dt>
                      <dd>{value}</dd>
                    </div>
                  ))}
                </dl>
              </article>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

export function Hero() {
  const { content } = useSitePreferences();
  const actions = content.hero.actions;
  return (
    <section className="hero">
      <div className="hero-ambient" aria-hidden="true">
        <img
          src="/media/ambient/abstract-desktop-product-background.webp"
          alt=""
          decoding="async"
          fetchPriority="high"
        />
        <div className="hero-ambient-veil" />
      </div>
      <div className="site-container hero-inner">
        <div className="hero-copy">
          <h1>{content.hero.title}</h1>
          <p className="hero-fact">{content.hero.fact}</p>
          <div className="hero-actions">
            {actions.map((action) => (
              <a
                key={action.label}
                className={`site-btn site-btn-${action.variant}`}
                {...actionProps(action)}
              >
                {action.icon === "github" ? <GithubIcon size={15} /> : null}
                {action.label}
              </a>
            ))}
          </div>
        </div>
        <div>
          <AppReplica />
        </div>
      </div>
    </section>
  );
}
