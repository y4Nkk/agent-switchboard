import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/noto-sans-sc/400.css";
import "@fontsource/noto-sans-sc/600.css";
import "@fontsource/noto-sans-sc/700.css";
import "../styles/tokens.css";
import "../styles/base.css";
import "../styles/boardui.css";
import "./tray.css";
import { TrayPanel } from "./TrayPanel";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode><TrayPanel /></React.StrictMode>,
);
