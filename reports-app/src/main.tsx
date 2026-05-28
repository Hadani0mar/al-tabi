import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "@/components/error-boundary";
import { applyTheme } from "@/lib/themes";
import "./App.css";
import "./themes/mihbar-theme.css";
import "./themes/elegant-luxury-theme.css";
import "./themes/cosmic-night-theme.css";

applyTheme("mihbar");
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <ErrorBoundary>
    <App />
  </ErrorBoundary>,
);
