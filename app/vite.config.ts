import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // The changelog shown in the app is the project's, one level up.
  server: { fs: { allow: ['..'] } },
})
