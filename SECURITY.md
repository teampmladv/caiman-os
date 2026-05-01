# Security Policy — Caimán OS

## Reportar vulnerabilidades

**No abrir issues públicos para vulnerabilidades de seguridad.**

Enviar a: team@caimanos.com

Incluir:
- Descripción de la vulnerabilidad
- Subsistema afectado (XDP, VMM, microseg, etc.)
- Pasos para reproducir
- Impacto potencial

Responderemos en **48 horas** con confirmación y plan de mitigación.

## Áreas de mayor riesgo

- `microseg/` — políticas XDP que controlan todo el tráfico de red
- `vmm/` — comunicación directa con `/dev/kvm`
- `install/scripts/` — scripts ejecutados como root
- `caiman-api/src/auth/` — autenticación JWT

## Versiones soportadas

| Versión | Soporte de seguridad |
|---------|---------------------|
| 0.1.x   | ✅ Activo            |
| < 0.1   | ❌ No soportado      |
