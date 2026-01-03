# DRFE-R Visualization Dashboard

A React + TypeScript frontend for visualizing the DRFE-R (Distributed Ricci Flow Embedding with Rendezvous Mechanism) network topology in the Poincaré disk model of hyperbolic space.

## Features

### Visualization Dashboard
- **Poincaré Disk Visualization**: Canvas-based rendering of network topology in hyperbolic space
- **Interactive Controls**: Zoom and pan to explore the network
- **Node Selection**: Click on nodes to view detailed information
- **Curvature Heatmap**: Visualize edge curvatures with color coding
- **Real-time Updates**: WebSocket support for live topology changes
- **Routing Animation**: Animated packet routing with mode indicators (Gravity/Pressure/Tree)
- **Export Panel**: Export visualizations to PDF/SVG for academic papers

### Chat Application (Demo Product)
- **Decentralized Chat**: Send messages routed via DRFE-R protocol
- **Room Support**: Create and join chat rooms with automatic routing
- **Routing Visualization**: See message routing paths in real-time
- **Topology Integration**: View network topology alongside chat
- **User Discovery**: See online users and their positions in hyperbolic space

## Tech Stack

- **React 19** - UI framework
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **D3.js** - Data visualization utilities
- **Canvas API** - High-performance rendering

## Getting Started

### Prerequisites

- Node.js 18+ 
- npm or yarn

### Installation

```bash
# Install dependencies
npm install

# Start development server
npm run dev
```

The app will be available at http://localhost:3000

### Build for Production

```bash
npm run build
```

Output will be in the `dist/` directory.

## Project Structure

```
frontend/
├── src/
│   ├── components/           # React components
│   │   ├── PoincareDisk.tsx      # Main visualization component
│   │   ├── NodeInspectionPanel.tsx # Node details panel
│   │   ├── ExportPanel.tsx       # Figure export for papers
│   │   ├── ChatPanel.tsx         # Chat UI component
│   │   ├── ChatApp.tsx           # Chat application with topology
│   │   └── index.ts
│   ├── hooks/               # Custom React hooks
│   │   ├── useWebSocket.ts       # WebSocket for topology updates
│   │   ├── useChatWebSocket.ts   # WebSocket for chat
│   │   ├── useRoutingAnimation.ts # Routing animation state
│   │   └── index.ts
│   ├── types/               # TypeScript type definitions
│   │   ├── index.ts              # Core types
│   │   └── chat.ts               # Chat-specific types
│   ├── utils/               # Utility functions
│   │   ├── hyperbolic.ts         # Hyperbolic geometry calculations
│   │   ├── exportFigure.ts       # Figure export utilities
│   │   └── generateLargeTopology.ts # Test data generation
│   ├── App.tsx              # Main visualization app
│   ├── ChatMain.tsx         # Chat application entry point
│   ├── App.css              # Application styles
│   ├── main.tsx             # Entry point
│   └── index.css            # Global styles
├── public/                  # Static assets
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## API Integration

The frontend is configured to connect to the Rust backend:

### Visualization Dashboard
- REST API: `http://localhost:8080/api/*`
- WebSocket: `ws://localhost:3001/ws` (topology updates)

### Chat Application
- WebSocket: `ws://localhost:8080/ws/{user_id}` (chat server)

### Environment Variables

Configure backend URLs via environment variables:

```bash
# .env.local
VITE_WS_URL=ws://localhost:3001/ws
VITE_CHAT_WS_URL=ws://localhost:8080/ws
```

## Usage

### Visualization Dashboard

The main dashboard shows the network topology in a Poincaré disk:

1. **Zoom**: Scroll to zoom in/out
2. **Pan**: Click and drag to move around
3. **Select Node**: Click on a node to see details
4. **Toggle Options**: Use sidebar checkboxes to show/hide features
5. **Run Demo**: Click "Run Demo" to see routing animation
6. **Export**: Use the Export Panel to save figures

### Chat Application

To use the chat application:

1. Import `ChatApp` component or use `ChatMain.tsx`
2. Enter a username to join
3. Create or join rooms
4. Send messages to users or rooms
5. Watch routing visualization as messages are delivered

```tsx
import { ChatApp } from './components';

function App() {
  return <ChatApp />;
}
```

## Development

### Available Scripts

- `npm run dev` - Start development server
- `npm run build` - Build for production
- `npm run lint` - Run ESLint
- `npm run preview` - Preview production build

### Adding New Components

1. Create component in `src/components/`
2. Export from `src/components/index.ts`
3. Add types to `src/types/`
4. Add styles to `src/App.css`

## License

MIT
