# Caimán OS — Dashboard UI

> Born in Cuba. Built for the cloud. Better than Rancher. Better than vSphere.

## Stack
- **React 18** + TypeScript + Vite
- **Zustand** + Immer — global state with live WebSocket updates
- **TanStack Query** — data fetching + caching
- **Framer Motion** — animations & transitions
- **@xyflow/react** — live cluster topology graph
- **Recharts** — real-time metrics charts
- **@xterm/xterm** — in-browser serial console
- **@monaco-editor/react** — YAML/policy editor
- **socket.io-client** — live cluster events
- **IBM Plex Mono + Syne** — typography

## Design tokens (CSS vars)
```css
--bg, --bg2, --bg3, --bg4     /* Dark backgrounds */
--border, --border2            /* Border hierarchy */
--green, --bright              /* Primary accent (caiman green) */
--dim, --text, --muted         /* Text hierarchy */
--amber, --red, --blue         /* Semantic colors */
```

## Quick start
```bash
npm install
npm run dev        # http://localhost:3000
npm run build      # Production build → dist/
```

## Environment
```env
VITE_API_URL=http://caiman-drs.caiman-system.svc:8765
VITE_WS_URL=http://caiman-drs.caiman-system.svc:8765
VITE_MOCK=true   # Use mock data (default when API unreachable)
```

## Key differentiators vs Rancher/vSphere
- ⌘K AI command bar — Claude directly in the UI
- Live topology graph with micro-seg policy overlay
- Zero-friction VM ops (1 click migrate/stop/console)
- Sub-second metric updates via WebSocket
- IBM Plex Mono "Mission Control" aesthetic
- Mobile-responsive (vSphere/Rancher are desktop-only)
