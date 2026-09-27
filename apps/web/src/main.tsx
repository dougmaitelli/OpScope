import "./styles/base.css";
import "./styles/controls.css";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App.tsx";
import { ApplicationClientProvider } from "./api/application-client.tsx";
import { WebAuthentication } from "./auth/WebAuthentication.tsx";

const root = document.getElementById("root");
if (!root) {
  throw new Error("application root is missing");
}

createRoot(root).render(
  <WebAuthentication>
    <ApplicationClientProvider>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </ApplicationClientProvider>
  </WebAuthentication>,
);
