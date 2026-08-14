import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// BASE_PATH is set by the GitHub Actions workflow to "/<repo-name>/" so the
// built site works under the GitHub Pages project path. Locally it stays "/".
export default defineConfig({
  base: process.env.BASE_PATH ?? "/",
  plugins: [react()],
});
