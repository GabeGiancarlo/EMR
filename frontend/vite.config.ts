import { defineConfig } from "vite";
import { readFile } from "node:fs/promises";
const apiTarget = process.env.NEXUS_API_PROXY || "http://127.0.0.1:8090";
export default defineConfig({
  plugins: [{name:"isolated-telehealth-qa",configureServer(server){
    if(!process.env.NEXUS_TELEHEALTH_QA_FIXTURE)return;
    server.middlewares.use("/__nexus_telehealth_fixture",async(req,res)=>{
      if(req.method!=="GET"){res.statusCode=405;res.end();return;}
      try{const fixture=await readFile(process.env.NEXUS_TELEHEALTH_QA_FIXTURE!,"utf8");res.setHeader("Content-Type","application/json");res.setHeader("Cache-Control","no-store");res.end(fixture);}catch{res.statusCode=404;res.end();}
    });
  }}],
  server: {
    port: 5173,
    strictPort: true,
    proxy: { "/api": apiTarget, "/healthz": apiTarget },
  },
  preview: { port: 4173 },
  build: { target: "es2022", sourcemap: false },
});
