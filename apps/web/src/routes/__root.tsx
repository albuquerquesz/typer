import { createRootRoute, HeadContent, Outlet } from "@tanstack/react-router";
import { Toaster } from "@typer/ui/components/sonner";
import { ThemeProvider } from "@/components/theme-provider";
import "../index.css";

export const Route = createRootRoute({
  component: RootComponent,
  head: () => ({
    meta: [{ title: "typer" }],
  }),
});

function RootComponent() {
  return (
    <>
      <HeadContent />
      <ThemeProvider
        attribute="class"
        defaultTheme="system"
        enableSystem
        disableTransitionOnChange
      >
        <Outlet />
        <Toaster richColors position="bottom-center" />
      </ThemeProvider>
    </>
  );
}
