# React + Vite Frontend Migration Plan

## Overview

Migrate the existing Jinja2/Bootstrap frontend to a modern React SPA with:
- **Build Tool**: Vite
- **UI Framework**: Tailwind CSS + shadcn/ui
- **State Management**: TanStack Query (server state) + Zustand (client state)
- **Deployment**: Separate SPA served independently from FastAPI backend

## Current State Analysis

### Templates to Migrate (17 views)
1. `login.html` - Authentication page
2. `dashboard_main.html` - Main dashboard with charts
3. `dashboard_clients.html` - Client management (DataTables)
4. `dashboard_rates.html` - Rate card management
5. `dashboard_rate_card_detail.html` - Rate card editing
6. `dashboard_did_inventory.html` - DID inventory management
7. `dashboard_dids.html` - DID assignment management
8. `dashboard_providers.html` - Provider management
9. `dashboard_invoices.html` - Invoice listing
10. `dashboard_invoice_detail.html` - Invoice detail view
11. `dashboard_cdr.html` - CDR records with search
12. `dashboard_billing.html` - Billing overview
13. `dashboard_financial_report.html` - Financial reports
14. `dashboard_monitor_system.html` - System monitoring (WebSocket)
15. `dashboard_switch_config.html` - Switch configuration
16. `dashboard_config.html` - System configuration
17. `dashboard_users.html` - User management

### API Endpoints (Already REST-ready)
- `/api/clients/` - CRUD operations
- `/api/rate-cards/` - CRUD + CSV upload
- `/api/did-inventory/` - CRUD + CSV upload
- `/api/dids/` - CRUD operations
- `/api/providers/` - CRUD operations
- `/api/invoices/` - CRUD + PDF generation
- `/api/cdr/` - Search and listing
- `/api/billing/` - Billing operations
- `/api/config/` - System configuration
- `/api/users/` - User management
- `/ws/system-status` - WebSocket for real-time monitoring

### JavaScript Logic to Migrate
- `rate_cards.js` - Rate card CRUD, CSV upload, inline editing
- `realtime.js` - Real-time call display
- `monitor-system.js` - WebSocket system monitoring

---

## Implementation Plan

### Phase 1: Project Setup

**1.1 Create Vite + React Project**
- Initialize project in `/opt/ApoloBilling/frontend/`
- Configure TypeScript
- Set up path aliases (`@/` for `src/`)

**1.2 Install Dependencies**
```
Core: react, react-dom, react-router-dom
UI: tailwindcss, @radix-ui/*, class-variance-authority, clsx, tailwind-merge
State: @tanstack/react-query, zustand
Utils: axios, date-fns, recharts, lucide-react
Dev: typescript, @types/*, eslint, prettier
```

**1.3 Configure shadcn/ui**
- Initialize with CLI
- Add base components: Button, Input, Card, Table, Dialog, Dropdown, etc.

**1.4 Set up Project Structure**
```
frontend/
├── src/
│   ├── components/
│   │   ├── ui/              # shadcn components
│   │   ├── layout/          # Sidebar, Header, etc.
│   │   └── shared/          # Reusable business components
│   ├── pages/               # Route pages
│   ├── hooks/               # Custom hooks
│   ├── stores/              # Zustand stores
│   ├── services/            # API clients
│   ├── types/               # TypeScript types
│   └── lib/                 # Utilities
├── public/
└── index.html
```

### Phase 2: Core Infrastructure

**2.1 API Client Setup**
- Create Axios instance with base URL configuration
- Add request/response interceptors for auth tokens
- Create typed API service functions for each endpoint

**2.2 Authentication**
- Create auth store with Zustand (user, token, login/logout)
- Create `useAuth` hook
- Implement protected route wrapper
- Create login page

**2.3 Layout Components**
- Create main layout with responsive sidebar
- Implement navigation menu matching current structure
- Add header with user info and logout
- Create breadcrumb component

**2.4 Routing Setup**
```typescript
Routes:
  /login
  /dashboard
  /clients
  /rates
  /rates/:id
  /did-inventory
  /dids
  /providers
  /invoices
  /invoices/:id
  /cdr
  /billing
  /reports/financial
  /monitor/system
  /config/switch
  /config/system
  /users
```

### Phase 3: Shared Components

**3.1 Data Table Component**
- Create reusable DataTable with TanStack Table
- Features: sorting, filtering, pagination, row selection
- Server-side pagination support

**3.2 Form Components**
- Form wrapper with react-hook-form + zod validation
- Standard form field components
- CSV upload component with drag-and-drop

**3.3 Modal/Dialog System**
- Confirmation dialogs
- Form modals for CRUD operations

**3.4 Charts**
- Recharts wrapper components for dashboard charts
- Call volume chart, revenue chart, etc.

### Phase 4: Page Implementation

**4.1 Dashboard Main** (`/dashboard`)
- Summary cards (clients, calls, revenue)
- Charts: call volume, revenue trends
- Recent activity list

**4.2 Clients Page** (`/clients`)
- Data table with all client fields
- Create/Edit modal
- Delete confirmation
- Status toggle

**4.3 Rate Cards** (`/rates`, `/rates/:id`)
- Rate card list with actions
- Detail page with rate entries table
- Inline editing for rates
- CSV import/export

**4.4 DID Management** (`/did-inventory`, `/dids`)
- DID inventory table with bulk import
- DID assignments table
- Filter by client, status

**4.5 Providers** (`/providers`)
- Provider list table
- CRUD operations

**4.6 Invoices** (`/invoices`, `/invoices/:id`)
- Invoice list with status filters
- Detail page with line items
- PDF download, send email actions
- Status management (draft, sent, paid)

**4.7 CDR Records** (`/cdr`)
- Search form with date range, client filter
- Results table with pagination
- Export functionality

**4.8 Billing** (`/billing`)
- Billing period management
- Generate invoices action
- Billing summary stats

**4.9 Financial Reports** (`/reports/financial`)
- Date range selector
- Revenue/cost breakdown
- Charts and summary tables

**4.10 System Monitor** (`/monitor/system`)
- WebSocket connection for real-time data
- System stats cards (CPU, memory, disk)
- Service status indicators
- Active calls display

**4.11 Configuration** (`/config/switch`, `/config/system`)
- Switch configuration form
- System settings form

**4.12 Users** (`/users`)
- User management table
- Role assignment
- Password reset

### Phase 5: Backend Adjustments

**5.1 CORS Configuration**
- Enable CORS for frontend dev server (localhost:5173)
- Configure production CORS settings

**5.2 API Response Standardization**
- Ensure consistent JSON response format
- Add pagination metadata to list endpoints
- Standardize error response format

**5.3 Authentication Endpoints**
- Add `/api/auth/login` endpoint returning JWT
- Add `/api/auth/me` endpoint for current user
- Add `/api/auth/refresh` if needed

### Phase 6: Testing & Polish

**6.1 Component Testing**
- Unit tests for utility functions
- Component tests for critical UI components

**6.2 Integration Testing**
- E2E tests for critical flows (login, CRUD operations)

**6.3 Polish**
- Loading states and skeletons
- Error boundaries
- Toast notifications
- Responsive design verification

### Phase 7: Build & Deployment

**7.1 Production Build**
- Configure Vite for production build
- Set up environment variables

**7.2 Deployment Options**
- Option A: Serve from nginx alongside FastAPI
- Option B: Deploy to CDN (Vercel, Netlify, etc.)
- Option C: FastAPI serves static build files

---

## File Changes Summary

### New Files (frontend/)
- ~50+ React components
- ~15 page components
- ~10 service/hook files
- Configuration files (vite.config.ts, tailwind.config.js, etc.)

### Backend Modifications
- `main.py` - Add CORS middleware
- Possibly add/modify auth endpoints
- No changes to existing API logic

### Files to Keep (for reference during migration)
- All `/templates/*.html` - Reference for UI layout
- All `/static/js/*.js` - Reference for business logic

### Files to Remove (after migration complete)
- `/templates/` directory
- `/static/` directory
- Template-related routes in FastAPI

---

## Estimated Scope

| Phase | Components/Tasks |
|-------|------------------|
| Phase 1 | Project setup, ~10 config files |
| Phase 2 | ~15 core components |
| Phase 3 | ~10 shared components |
| Phase 4 | ~15 page components, ~20 feature components |
| Phase 5 | ~5 backend file modifications |
| Phase 6 | Tests and polish |
| Phase 7 | Build configuration |

---

## Questions Answered
- **UI Library**: Tailwind CSS + shadcn/ui
- **State Management**: TanStack Query + Zustand
- **Deployment**: Separate SPA

Ready for implementation upon approval.
