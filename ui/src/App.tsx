import React, { useState, useEffect } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Toaster } from 'react-hot-toast'
import { TopNav }         from './components/layout/TopNav'
import { Sidebar }        from './components/layout/Sidebar'
import { CommandBar }     from './components/ai/CommandBar'
import { VmDetailPanel }  from './components/vm/VmDetailPanel'
import { NotificationStack } from './components/ui/ActivityFeed'
import { useClusterStore }   from './store/cluster'
import { fetchVms, fetchNodes } from './api/client'
import { buildSnapshot } from './api/transform'

// Framework
import {
  ActionBusProvider,
  ProgressPanel,
  useContextShortcuts,
} from './framework'

const OverviewPage = React.lazy(() => import('./pages/Overview'))
const TopologyPage = React.lazy(() => import('./pages/Topology'))
const VmsPage      = React.lazy(() => import('./pages/VMs'))
const DrsPage      = React.lazy(() => import('./pages/DRS'))
const MicrosegPage = React.lazy(() => import('./pages/Microseg'))
const StoragePage  = React.lazy(() => import('./pages/Storage'))
const GpuPage      = React.lazy(() => import('./pages/GPU'))
const LoginPage   = React.lazy(() => import('./pages/Login'))

const qc = new QueryClient({
  defaultOptions: { queries: { staleTime: 10_000, retry: 1 } },
})

function AppInner() {
  const [tab, setTab] = useState('overview')
  const { setSnapshot, addNotification } = useClusterStore(s => ({
    setSnapshot:     s.setSnapshot,
    addNotification: s.addNotification,
  }))

  useEffect(() => {
    const load = async () => {
      try {
        const [vms, nodes] = await Promise.all([
          fetchVms().catch(() => []),
          fetchNodes().catch(() => []),
        ])
        setSnapshot(buildSnapshot(vms, nodes))
      } catch (e) {
        console.error('[caiman] failed to load cluster state', e)
      }
    }
    load()
    const interval = setInterval(load, 5000)
    return () => clearInterval(interval)
  }, [])

  useContextShortcuts()   // ← global keyboard shortcuts via framework

  const PAGE: Record<string, React.ReactNode> = {
    overview: <OverviewPage />,
    topology: <TopologyPage />,
    vms:      <VmsPage />,
    drs:      <DrsPage />,
    microseg: <MicrosegPage />,
    storage:  <StoragePage />,
    gpu:      <GpuPage />,
  }

  return (
    <div className="h-screen flex flex-col overflow-hidden select-none">
      <TopNav activeTab={tab} onTabChange={setTab} />
      <div className="flex flex-1 overflow-hidden relative">
        <Sidebar activeTab={tab} onTabChange={setTab} />
        <main className="flex-1 overflow-hidden flex flex-col relative">
          <React.Suspense fallback={
            <div className="flex-1 flex items-center justify-center">
              <div className="w-6 h-6 rounded-full border-2 border-caiman-green
                              border-t-transparent animate-spin" />
            </div>
          }>
            {PAGE[tab] ?? <OverviewPage />}
          </React.Suspense>
        </main>
        <VmDetailPanel />
      </div>
      <footer className="h-6 bg-caiman-bg2 border-t border-caiman-border
                         flex items-center px-3 gap-4 text-[8px] text-caiman-dim
                         tracking-[1px] flex-shrink-0">
        <span>caiman_net.ko <span className="text-caiman-green">ACTIVE</span></span>
        <span>XDP <span className="text-caiman-green">ATTACHED</span></span>
        <span>microseg <span className="text-caiman-green">ENFORCING</span></span>
        <span>DRS <span className="text-caiman-green">FULLY AUTO</span></span>
        <span className="ml-auto text-caiman-green">Born in Cuba · Built for the cloud</span>
        <span className="animate-[blink_1s_step-end_infinite]">▮</span>
      </footer>

      {/* Framework overlays */}
      <CommandBar onQuery={(q) => addNotification('info', 'Claude', q)} />
      <ProgressPanel />
      <NotificationStack />
      <Toaster
        position="bottom-right"
        toastOptions={{
          style: {
            background: '#0a150a',
            border: '1px solid #2e7d32',
            color: '#c8e6c9',
            fontFamily: 'IBM Plex Mono',
            fontSize: '11px',
          },
        }}
      />
    </div>
  )
}

export default function App() {
  const [token, setToken] = useState<string | null>(() => localStorage.getItem('caiman_token'))

  useEffect(() => {
    const onLogout = () => setToken(null)
    window.addEventListener('caiman:logout', onLogout)
    return () => window.removeEventListener('caiman:logout', onLogout)
  }, [])

  if (!token) {
    return (
      <QueryClientProvider client={qc}>
        <React.Suspense fallback={<div className="min-h-screen bg-caiman-bg" />}>
          <LoginPage onSuccess={(t) => setToken(t)} />
        </React.Suspense>
      </QueryClientProvider>
    )
  }

  return (
    <QueryClientProvider client={qc}>
      <ActionBusProvider>
        <AppInner />
      </ActionBusProvider>
    </QueryClientProvider>
  )
}
