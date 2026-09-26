import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App.tsx";
import { ApplicationClientProvider } from "./api/application-client.tsx";
import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("application root is missing");
}

createRoot(root).render(
  <ApplicationClientProvider>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </ApplicationClientProvider>,
);
