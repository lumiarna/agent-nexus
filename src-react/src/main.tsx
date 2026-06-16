import React from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from "sonner";
import App from "@/App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
    <Toaster
      position="bottom-center"
      duration={2400}
      toastOptions={{
        unstyled: true,
        classNames: {
          toast:
            "mx-auto flex items-center justify-center rounded-full bg-nexus-ink px-[18px] py-[11px] text-[12.5px] font-semibold text-nexus-bg shadow-[0_10px_30px_rgba(50,40,25,.32)]",
        },
      }}
    />
  </React.StrictMode>,
);
