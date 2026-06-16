import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import App from "@/App";
import "./index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: false,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
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
    </QueryClientProvider>
  </React.StrictMode>,
);
