import { useApp } from '../store.jsx'
import { C, Card, Badge, SectionHeader, Scroller, MetricCard } from '../components/UI.jsx'
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'

const RATES = { vcpu: 0.02, ram: 0.004, disk: 0.001, gpu: 2.5 }
const MONTHS = ['Jan','Feb','Mar','Apr','May']
const billingData = MONTHS.map(m => ({ m, prod: Math.round(Math.random()*200+300), dev: Math.round(Math.random()*80+50), ml: Math.round(Math.random()*300+100), ops: Math.round(Math.random()*20+10) }))

export default function Billing() {
  const { tenants, vms } = useApp()
  const totalCost = tenants.reduce((a,t) => {
    const tvms = vms.filter(v => v.tenant === t.name && v.status === 'RUNNING')
    const cost = tvms.reduce((s,v) => s + v.cpus*RATES.vcpu*24*30 + v.mem/1024*RATES.ram*24*30 + v.disk*RATES.disk*24*30 + (v.gpu?RATES.gpu*24*30:0), 0)
    return a + cost
  }, 0)

  return (
    <Scroller>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 8, marginBottom: 16 }}>
        <MetricCard label="Monthly Total" value={`$${Math.round(totalCost)}`} sub="all tenants" />
        <MetricCard label="Projected" value={`$${Math.round(totalCost*1.08)}`} sub="+8% vs last month" color={C.amb} />
        <MetricCard label="Savings vs Cloud" value="68%" sub="vs AWS equivalent" color={C.blu} />
      </div>

      <SectionHeader title="Cost by Tenant" />
      <Card style={{ marginBottom: 16 }}>
        <ResponsiveContainer width="100%" height={180}>
          <BarChart data={billingData}>
            <XAxis dataKey="m" tick={{ fill: C.dim, fontSize: 10 }} />
            <YAxis tick={{ fill: C.dim, fontSize: 10 }} />
            <Tooltip contentStyle={{ background: C.bg3, border: `1px solid ${C.brd}`, fontSize: 10 }} />
            <Bar dataKey="prod" fill="#22c55e" stackId="a" />
            <Bar dataKey="ml" fill="#a78bfa" stackId="a" />
            <Bar dataKey="dev" fill="#60a5fa" stackId="a" />
            <Bar dataKey="ops" fill="#fbbf24" stackId="a" />
          </BarChart>
        </ResponsiveContainer>
      </Card>

      <SectionHeader title="Tenant Chargeback" />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {tenants.map(t => {
          const tvms = vms.filter(v => v.tenant === t.name && v.status === 'RUNNING')
          const cost = tvms.reduce((s,v) => s + v.cpus*RATES.vcpu*24*30 + v.mem/1024*RATES.ram*24*30 + v.disk*RATES.disk*24*30 + (v.gpu?RATES.gpu*24*30:0), 0)
          const pct = totalCost > 0 ? Math.round(cost/totalCost*100) : 0
          return (
            <Card key={t.id} style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
              <div style={{ width: 10, height: 10, borderRadius: '50%', background: t.color, flexShrink: 0 }} />
              <div style={{ width: 80 }}><Badge color={t.color}>{t.name}</Badge></div>
              <div style={{ flex: 1 }}>
                <div style={{ height: 4, background: C.brd, borderRadius: 2, marginBottom: 2 }}>
                  <div style={{ height: '100%', width: `${pct}%`, background: t.color, borderRadius: 2 }} />
                </div>
                <div style={{ fontSize: 10, color: C.dim }}>{tvms.length} VMs · {t.vcpus} vCPU · {t.ram/1024}GiB RAM</div>
              </div>
              <div style={{ textAlign: 'right' }}>
                <div style={{ fontSize: 16, fontFamily: 'Syne', fontWeight: 800, color: t.color }}>${Math.round(cost)}</div>
                <div style={{ fontSize: 9, color: C.dim }}>this month</div>
              </div>
            </Card>
          )
        })}
      </div>
    </Scroller>
  )
}
