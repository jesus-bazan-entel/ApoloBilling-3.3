# ApoloBilling Frontend

Modern React + TypeScript frontend for the ApoloBilling system - a professional real-time billing platform for telecommunications.

## Technology Stack

- **React 19** - Latest React with concurrent features
- **TypeScript** - Type-safe development
- **Vite 7** - Lightning-fast build tool
- **Tailwind CSS v4** - Utility-first CSS framework
- **React Router v7** - Client-side routing
- **TanStack React Query** - Server state management
- **Axios** - HTTP client
- **Recharts** - Charts and visualizations
- **Lucide React** - Modern icon library
- **date-fns** - Date formatting utilities

## Features

### Real-Time Monitoring
- **Dashboard** - Live statistics, active calls, and revenue tracking
- **Active Calls** - WebSocket-powered real-time call monitoring
- **CDR (Call Detail Records)** - Complete call history with filtering and export

### Account Management
- **Accounts** - CRUD for prepaid/postpaid accounts
- **Balance Management** - Anexo recharges with quick amounts
- **Transaction Audit** - Complete balance transaction history

### Rating & Billing
- **Zones** - Geographic zone configuration
- **Rate Cards** - Tariff management with LPM (Longest Prefix Match)
- **Rate Lookup** - Test the LPM algorithm

## Project Structure

```
frontend/
├── src/
│   ├── api/
│   │   └── client.ts              # API client with all endpoints
│   ├── components/
│   │   ├── Layout.tsx             # Main layout with sidebar
│   │   ├── StatCard.tsx           # Statistics card component
│   │   ├── DataTable.tsx          # Reusable data table with pagination
│   │   └── Badge.tsx              # Badge component for status indicators
│   ├── hooks/
│   │   └── useWebSocket.ts        # WebSocket hook for real-time updates
│   ├── pages/
│   │   ├── Dashboard.tsx          # Main dashboard
│   │   ├── ActiveCalls.tsx        # Real-time call monitoring
│   │   ├── CDR.tsx                # Call detail records
│   │   ├── Accounts.tsx           # Account management
│   │   ├── Balance.tsx            # Balance/Saldo management
│   │   ├── Zones.tsx              # Geographic zones
│   │   └── Rates.tsx              # Rate cards & tariffs
│   ├── types/
│   │   └── index.ts               # TypeScript type definitions
│   ├── App.tsx                    # Main app component with routing
│   ├── main.tsx                   # Entry point
│   └── index.css                  # Global styles with Tailwind
├── public/
├── dist/                          # Build output
├── index.html
├── vite.config.ts                 # Vite configuration with proxy
├── tsconfig.json                  # TypeScript configuration
└── package.json
```

## Quick Start

### Prerequisites
- Node.js 18+ and npm
- Backend API running on http://127.0.0.1:8000

### Installation

```bash
cd /opt/ApoloBilling/frontend
npm install
```

### Development

```bash
npm run dev
```

Access the app at: http://localhost:3000

The development server includes:
- Hot Module Replacement (HMR)
- API proxy to backend (port 9000)
- WebSocket proxy for real-time updates

### Production Build

```bash
npm run build
```

Output: `/opt/ApoloBilling/frontend/dist/`

### Preview Production Build

```bash
npm run preview
```

## API Integration

The frontend connects to the Rust billing engine backend running on port 9000.

### API Endpoints

All endpoints are configured in `src/api/client.ts`:

- **Health**: `GET /api/v1/health`
- **Stats**: `GET /api/v1/stats`
- **Accounts**: `GET|POST|PUT /api/v1/accounts`
- **Anexos**: `GET /api/v1/anexos`, `POST /api/v1/anexos/{numero}/recharge`
- **Zones**: `GET|POST /api/v1/zones`
- **Rate Cards**: `GET|POST|PUT|DELETE /api/v1/rate-cards`
- **CDRs**: `GET /api/v1/cdrs` (with pagination & filters)
- **Active Calls**: `GET /api/v1/calls/active`
- **Reservations**: `GET /api/v1/reservations/active`
- **Rate Lookup**: `GET /api/v1/rate-lookup/{destination}`

### WebSocket

Real-time updates via WebSocket at `ws://localhost:9000/ws`

Message types:
- `call_start` - New call started
- `call_update` - Call status updated
- `call_end` - Call ended
- `stats_update` - Dashboard stats updated

## Component Usage

### DataTable

Reusable table component with pagination:

```tsx
import DataTable from '../components/DataTable'

<DataTable
  columns={[
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name', render: (item) => <b>{item.name}</b> }
  ]}
  data={items}
  loading={isLoading}
  emptyMessage="No data available"
  pagination={{
    page: currentPage,
    totalPages: 10,
    onPageChange: setCurrentPage
  }}
/>
```

### StatCard

Dashboard statistics card:

```tsx
import StatCard from '../components/StatCard'
import { Users } from 'lucide-react'

<StatCard
  title="Active Users"
  value={150}
  subtitle="Last 24 hours"
  icon={Users}
  color="blue"
  trend={{ value: 12, isPositive: true }}
/>
```

### Badge

Status indicators:

```tsx
import Badge from '../components/Badge'

<Badge variant="success">Active</Badge>
<Badge variant="warning">Pending</Badge>
<Badge variant="error">Error</Badge>
```

## Performance Optimizations

### Code Splitting
- Route-based code splitting via React Router
- Lazy loading of heavy components

### Caching
- React Query with 5s stale time
- Optimistic updates for mutations

### Bundle Size
- Tree-shaking enabled
- CSS purging via Tailwind
- Production build: ~380KB JS + ~22KB CSS (gzipped: ~116KB + ~5KB)

### Real-Time Updates
- WebSocket reconnection logic
- Fallback to polling when WS disconnected
- Auto-refresh intervals: 5s (calls), 10s (stats)

## Accessibility

### WCAG Compliance
- Semantic HTML structure
- ARIA labels on interactive elements
- Keyboard navigation support
- Focus indicators
- Color contrast ratios meet AA standards

### Screen Reader Support
- Descriptive labels
- Status announcements
- Form error messages

## Responsive Design

Mobile-first approach with breakpoints:
- Mobile: < 768px
- Tablet: 768px - 1024px
- Desktop: > 1024px

### Layout
- Fixed sidebar on desktop (256px)
- Collapsible menu on mobile
- Responsive grid layouts
- Touch-friendly tap targets (min 44x44px)

## Environment Variables

No environment variables needed - all configuration is in `vite.config.ts`:

```typescript
server: {
  port: 3000,
  proxy: {
    '/api': 'http://127.0.0.1:8000',
    '/ws': 'ws://127.0.0.1:9000'
  }
}
```

## Development Tips

### Type Safety
All API responses are typed. Update `src/types/index.ts` when backend schemas change.

### React Query Dev Tools
Add for debugging:
```bash
npm install @tanstack/react-query-devtools
```

### Debugging WebSocket
Check browser console for WebSocket connection status and messages.

### Hot Reload
Vite HMR preserves React state during development. Full reload on:
- Config changes
- New dependencies
- Type errors

## Troubleshooting

### Build Errors

**TypeScript errors:**
```bash
npm run build
```
Fix type errors in reported files.

**CSS not loading:**
Ensure Tailwind CSS v4 is properly configured with `@tailwindcss/vite` plugin.

### Runtime Errors

**API connection failed:**
- Verify backend is running on port 9000
- Check CORS configuration
- Review browser console for detailed errors

**WebSocket not connecting:**
- Ensure backend WebSocket endpoint is active
- Check firewall rules
- Verify proxy configuration in vite.config.ts

**Data not loading:**
- Check React Query cache
- Verify API endpoints match backend
- Review network tab in DevTools

## Contributing

When adding new features:

1. Add TypeScript types to `src/types/index.ts`
2. Add API methods to `src/api/client.ts`
3. Create page component in `src/pages/`
4. Add route to `src/App.tsx`
5. Update sidebar navigation in `src/components/Layout.tsx`
6. Test mobile responsiveness
7. Verify accessibility
8. Run `npm run build` before committing

## License

Part of ApoloBilling system - Internal use only

## Support

For issues or questions:
- Check browser console for errors
- Review API endpoint responses
- Verify backend is running and accessible
- Check this README for configuration details
