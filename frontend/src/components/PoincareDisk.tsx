/**
 * Poincaré Disk Visualization Component
 * Renders the hyperbolic network topology using Canvas API
 * 
 * Features:
 * - Canvas-based renderer for hyperbolic space
 * - Node positioning in Poincaré disk
 * - Edge rendering with geodesic arcs
 * - Zoom and pan controls
 * - Curvature heatmap overlay
 * - Real-time packet routing animation
 * - Mode indicators (Gravity/Pressure/Tree)
 */

import { useRef, useEffect, useState, useCallback, useMemo } from 'react';
import type { NetworkTopology, NetworkNode, RenderOptions, PacketAnimation, RoutingMode } from '../types';
import { toCanvasCoords, curvatureToColor, calculateGeodesic } from '../utils/hyperbolic';

interface PoincareDiskProps {
  topology: NetworkTopology;
  selectedNode?: string;
  onNodeSelect?: (nodeId: string | null) => void;
  options?: Partial<RenderOptions>;
  /** Active packet animations */
  animations?: PacketAnimation[];
  /** Show routing mode indicator */
  showModeIndicator?: boolean;
  /** Current active routing mode (for display) */
  activeMode?: RoutingMode | null;
}

const DEFAULT_OPTIONS: RenderOptions = {
  width: 600,
  height: 600,
  showEdges: true,
  showLabels: true,
  showCurvature: false,
  animateRouting: true,
};

/** Colors for different routing modes */
const MODE_COLORS: Record<RoutingMode, string> = {
  gravity: '#00ff88',   // Green - moving toward destination
  pressure: '#ff8800',  // Orange - escaping local minimum
  tree: '#ff0088',      // Pink - using spanning tree fallback
};

/** Mode display names */
const MODE_NAMES: Record<RoutingMode, string> = {
  gravity: 'Gravity',
  pressure: 'Pressure',
  tree: 'Tree',
};

// Zoom constraints
const MIN_ZOOM = 0.1;
const MAX_ZOOM = 10;
const ZOOM_SENSITIVITY = 0.001;

/** Animation duration for packet movement (ms) */
// const PACKET_ANIMATION_DURATION = 500; // Reserved for future use

export function PoincareDisk({
  topology,
  selectedNode,
  onNodeSelect,
  options: userOptions,
  animations = [],
  showModeIndicator = true,
  activeMode = null,
}: PoincareDiskProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [scale, setScale] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);

  // Memoize options to prevent unnecessary re-renders
  const options = useMemo(
    () => ({ ...DEFAULT_OPTIONS, ...userOptions }),
    [userOptions]
  );

  // Memoize node lookup map
  const nodeMap = useMemo(() => {
    const map = new Map<string, NetworkNode>();
    topology.nodes.forEach(node => map.set(node.id, node));
    return map;
  }, [topology.nodes]);

  // Draw the visualization
  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const { width, height } = options;
    const centerX = width / 2;
    const centerY = height / 2;
    const baseRadius = Math.min(width, height) / 2 - 20;

    // Clear canvas with background
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, width, height);

    // Save context for transformations
    ctx.save();
    ctx.translate(offset.x, offset.y);

    // Draw concentric reference circles (hyperbolic distance markers)
    drawReferenceCircles(ctx, centerX, centerY, baseRadius, scale);

    // Draw Poincaré disk boundary
    ctx.beginPath();
    ctx.arc(centerX, centerY, baseRadius * scale, 0, 2 * Math.PI);
    ctx.strokeStyle = '#6a6a9a';
    ctx.lineWidth = 2;
    ctx.stroke();
    ctx.fillStyle = '#16213e';
    ctx.fill();

    // Draw coordinate axes (faint)
    drawAxes(ctx, centerX, centerY, baseRadius, scale);

    // Draw edges as geodesics
    if (options.showEdges) {
      drawEdges(ctx, topology, nodeMap, options, width, height, scale, offset);
    }

    // Draw nodes
    drawNodes(ctx, topology, selectedNode, hoveredNode, options, width, height, scale, offset);

    // Draw packet animations
    if (options.animateRouting && animations.length > 0) {
      drawPacketAnimations(ctx, animations, nodeMap, width, height, scale, offset);
    }

    ctx.restore();

    // Draw zoom indicator (fixed position)
    drawZoomIndicator(ctx, scale, width, height);

    // Draw curvature legend when heatmap is enabled (fixed position)
    if (options.showCurvature) {
      drawCurvatureLegend(ctx, width);
    }

    // Draw mode indicator (fixed position)
    if (showModeIndicator && activeMode) {
      drawModeIndicator(ctx, activeMode, width);
    }
  }, [topology, selectedNode, hoveredNode, options, scale, offset, nodeMap, animations, showModeIndicator, activeMode]);

  // Redraw on changes
  useEffect(() => {
    draw();
  }, [draw]);

  // Handle mouse wheel for zoom
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    // Calculate zoom factor
    const delta = -e.deltaY * ZOOM_SENSITIVITY;
    const newScale = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, scale * (1 + delta)));
    const scaleFactor = newScale / scale;

    // Zoom towards mouse position
    const newOffsetX = mouseX - (mouseX - offset.x) * scaleFactor;
    const newOffsetY = mouseY - (mouseY - offset.y) * scaleFactor;

    setScale(newScale);
    setOffset({ x: newOffsetX, y: newOffsetY });
  }, [scale, offset]);

  // Handle mouse down for pan
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 0) { // Left click
      setIsDragging(true);
      setDragStart({ x: e.clientX - offset.x, y: e.clientY - offset.y });
    }
  }, [offset]);

  // Handle mouse move for pan and hover
  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    if (isDragging) {
      setOffset({
        x: e.clientX - dragStart.x,
        y: e.clientY - dragStart.y,
      });
      return;
    }

    // Check for node hover
    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    let foundNode: string | null = null;
    for (const node of topology.nodes) {
      const pos = toCanvasCoords(
        node.coordinate, options.width, options.height, scale, offset.x, offset.y
      );
      const dist = Math.sqrt((mouseX - pos.x) ** 2 + (mouseY - pos.y) ** 2);
      if (dist < 12) {
        foundNode = node.id;
        break;
      }
    }
    setHoveredNode(foundNode);
  }, [isDragging, dragStart, topology.nodes, options, scale, offset]);

  // Handle mouse up
  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  // Handle click for node selection
  const handleClick = useCallback((e: React.MouseEvent) => {
    if (!onNodeSelect) return;

    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const clickX = e.clientX - rect.left;
    const clickY = e.clientY - rect.top;

    // Find clicked node
    for (const node of topology.nodes) {
      const pos = toCanvasCoords(
        node.coordinate, options.width, options.height, scale, offset.x, offset.y
      );
      const dist = Math.sqrt((clickX - pos.x) ** 2 + (clickY - pos.y) ** 2);
      if (dist < 12) {
        onNodeSelect(node.id);
        return;
      }
    }
    onNodeSelect(null);
  }, [onNodeSelect, topology.nodes, options, scale, offset]);

  // Handle double click to reset view
  const handleDoubleClick = useCallback(() => {
    setScale(1);
    setOffset({ x: 0, y: 0 });
  }, []);

  // Reset view function
  const resetView = useCallback(() => {
    setScale(1);
    setOffset({ x: 0, y: 0 });
  }, []);

  return (
    <div className="poincare-disk-container">
      <canvas
        ref={canvasRef}
        width={options.width}
        height={options.height}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
        style={{ 
          cursor: isDragging ? 'grabbing' : (hoveredNode ? 'pointer' : 'grab'),
          borderRadius: '8px',
        }}
      />
      <div className="poincare-controls">
        <button onClick={() => setScale(s => Math.min(MAX_ZOOM, s * 1.2))} title="Zoom In">+</button>
        <button onClick={() => setScale(s => Math.max(MIN_ZOOM, s / 1.2))} title="Zoom Out">−</button>
        <button onClick={resetView} title="Reset View">⟲</button>
      </div>
    </div>
  );
}

/**
 * Draw concentric reference circles showing hyperbolic distance from origin
 */
function drawReferenceCircles(
  ctx: CanvasRenderingContext2D,
  centerX: number,
  centerY: number,
  baseRadius: number,
  scale: number
) {
  ctx.strokeStyle = '#2a2a4a';
  ctx.lineWidth = 0.5;
  ctx.setLineDash([4, 4]);

  // Draw circles at Euclidean radii 0.25, 0.5, 0.75
  [0.25, 0.5, 0.75].forEach(r => {
    ctx.beginPath();
    ctx.arc(centerX, centerY, r * baseRadius * scale, 0, 2 * Math.PI);
    ctx.stroke();
  });

  ctx.setLineDash([]);
}

/**
 * Draw coordinate axes
 */
function drawAxes(
  ctx: CanvasRenderingContext2D,
  centerX: number,
  centerY: number,
  baseRadius: number,
  scale: number
) {
  ctx.strokeStyle = '#3a3a5a';
  ctx.lineWidth = 0.5;

  // Horizontal axis
  ctx.beginPath();
  ctx.moveTo(centerX - baseRadius * scale, centerY);
  ctx.lineTo(centerX + baseRadius * scale, centerY);
  ctx.stroke();

  // Vertical axis
  ctx.beginPath();
  ctx.moveTo(centerX, centerY - baseRadius * scale);
  ctx.lineTo(centerX, centerY + baseRadius * scale);
  ctx.stroke();
}

/**
 * Draw edges as geodesics (hyperbolic arcs)
 */
function drawEdges(
  ctx: CanvasRenderingContext2D,
  topology: NetworkTopology,
  nodeMap: Map<string, NetworkNode>,
  options: RenderOptions,
  width: number,
  height: number,
  scale: number,
  offset: { x: number; y: number }
) {
  topology.edges.forEach(edge => {
    const sourceNode = nodeMap.get(edge.source);
    const targetNode = nodeMap.get(edge.target);
    if (!sourceNode || !targetNode) return;

    // Calculate geodesic points
    const geodesicPoints = calculateGeodesic(
      sourceNode.coordinate,
      targetNode.coordinate,
      20
    );

    if (geodesicPoints.length < 2) return;

    // Convert to canvas coordinates
    const canvasPoints = geodesicPoints.map(p =>
      toCanvasCoords(p, width, height, scale, offset.x, offset.y)
    );

    // Draw the geodesic path
    ctx.beginPath();
    ctx.moveTo(canvasPoints[0].x, canvasPoints[0].y);
    for (let i = 1; i < canvasPoints.length; i++) {
      ctx.lineTo(canvasPoints[i].x, canvasPoints[i].y);
    }

    if (options.showCurvature && edge.curvature !== undefined) {
      const color = curvatureToColor(edge.curvature);
      
      // Draw glow effect for curvature visualization
      ctx.save();
      ctx.shadowColor = color;
      ctx.shadowBlur = 6;
      ctx.strokeStyle = color;
      ctx.lineWidth = 3;
      ctx.stroke();
      ctx.restore();
      
      // Draw solid line on top
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.stroke();
    } else {
      ctx.strokeStyle = '#4a4a7a';
      ctx.lineWidth = 1;
      ctx.stroke();
    }
  });
}

/**
 * Draw nodes
 */
function drawNodes(
  ctx: CanvasRenderingContext2D,
  topology: NetworkTopology,
  selectedNode: string | undefined,
  hoveredNode: string | null,
  options: RenderOptions,
  width: number,
  height: number,
  scale: number,
  offset: { x: number; y: number }
) {
  topology.nodes.forEach(node => {
    const pos = toCanvasCoords(
      node.coordinate, width, height, scale, offset.x, offset.y
    );

    const isSelected = node.id === selectedNode;
    const isHovered = node.id === hoveredNode;
    const nodeRadius = isSelected ? 8 : (isHovered ? 7 : 5);

    // Draw node glow for selected/hovered
    if (isSelected || isHovered) {
      ctx.beginPath();
      ctx.arc(pos.x, pos.y, nodeRadius + 4, 0, 2 * Math.PI);
      ctx.fillStyle = isSelected ? 'rgba(0, 255, 136, 0.3)' : 'rgba(79, 195, 247, 0.3)';
      ctx.fill();
    }

    // Draw node
    ctx.beginPath();
    ctx.arc(pos.x, pos.y, nodeRadius, 0, 2 * Math.PI);
    
    if (isSelected) {
      ctx.fillStyle = '#00ff88';
    } else if (isHovered) {
      ctx.fillStyle = '#7fd8ff';
    } else if (node.isOnline) {
      ctx.fillStyle = '#4fc3f7';
    } else {
      ctx.fillStyle = '#666';
    }
    ctx.fill();

    // Draw node border
    ctx.strokeStyle = isSelected ? '#00ff88' : (isHovered ? '#7fd8ff' : '#fff');
    ctx.lineWidth = isSelected || isHovered ? 2 : 1;
    ctx.stroke();

    // Draw label
    if (options.showLabels && scale > 0.5) {
      ctx.fillStyle = '#fff';
      ctx.font = `${Math.max(9, 11 * scale)}px monospace`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'bottom';
      ctx.fillText(node.id.slice(0, 8), pos.x, pos.y - nodeRadius - 4);
    }
  });
}

/**
 * Draw zoom indicator
 */
function drawZoomIndicator(
  ctx: CanvasRenderingContext2D,
  scale: number,
  width: number,
  height: number
) {
  const text = `${Math.round(scale * 100)}%`;
  ctx.fillStyle = 'rgba(0, 0, 0, 0.5)';
  ctx.fillRect(width - 60, height - 30, 55, 25);
  ctx.fillStyle = '#aaa';
  ctx.font = '12px monospace';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, width - 32, height - 17);
}

/**
 * Draw curvature legend (color scale) when curvature heatmap is enabled
 */
function drawCurvatureLegend(
  ctx: CanvasRenderingContext2D,
  width: number
) {
  const legendWidth = 20;
  const legendHeight = 150;
  const legendX = width - 40;
  const legendY = 60;
  const padding = 5;

  // Draw background
  ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
  ctx.fillRect(legendX - padding - 35, legendY - padding - 20, legendWidth + padding * 2 + 45, legendHeight + padding * 2 + 40);
  ctx.strokeStyle = '#4a4a7a';
  ctx.lineWidth = 1;
  ctx.strokeRect(legendX - padding - 35, legendY - padding - 20, legendWidth + padding * 2 + 45, legendHeight + padding * 2 + 40);

  // Draw title
  ctx.fillStyle = '#fff';
  ctx.font = 'bold 10px monospace';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText('Curvature', legendX - 10, legendY - 8);

  // Draw gradient bar
  const gradient = ctx.createLinearGradient(legendX, legendY, legendX, legendY + legendHeight);
  gradient.addColorStop(0, '#ff6666');     // Positive curvature (top) - red
  gradient.addColorStop(0.5, '#ffffff');   // Zero curvature (middle) - white
  gradient.addColorStop(1, '#6666ff');     // Negative curvature (bottom) - blue

  ctx.fillStyle = gradient;
  ctx.fillRect(legendX, legendY, legendWidth, legendHeight);

  // Draw border around gradient
  ctx.strokeStyle = '#888';
  ctx.lineWidth = 1;
  ctx.strokeRect(legendX, legendY, legendWidth, legendHeight);

  // Draw scale labels
  ctx.fillStyle = '#ccc';
  ctx.font = '9px monospace';
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';

  // Positive label (top)
  ctx.fillText('+1.0', legendX - 30, legendY + 5);
  ctx.fillText('(+)', legendX - 25, legendY + 15);

  // Zero label (middle)
  ctx.fillText(' 0.0', legendX - 30, legendY + legendHeight / 2);

  // Negative label (bottom)
  ctx.fillText('-1.0', legendX - 30, legendY + legendHeight - 5);
  ctx.fillText('(-)', legendX - 25, legendY + legendHeight - 15);

  // Draw tick marks
  ctx.strokeStyle = '#888';
  ctx.lineWidth = 1;
  
  // Top tick
  ctx.beginPath();
  ctx.moveTo(legendX - 3, legendY);
  ctx.lineTo(legendX, legendY);
  ctx.stroke();

  // Middle tick
  ctx.beginPath();
  ctx.moveTo(legendX - 3, legendY + legendHeight / 2);
  ctx.lineTo(legendX, legendY + legendHeight / 2);
  ctx.stroke();

  // Bottom tick
  ctx.beginPath();
  ctx.moveTo(legendX - 3, legendY + legendHeight);
  ctx.lineTo(legendX, legendY + legendHeight);
  ctx.stroke();
}

/**
 * Draw packet animations along geodesic paths
 */
function drawPacketAnimations(
  ctx: CanvasRenderingContext2D,
  animations: PacketAnimation[],
  nodeMap: Map<string, NetworkNode>,
  width: number,
  height: number,
  scale: number,
  offset: { x: number; y: number }
) {
  animations.forEach(animation => {
    const fromNode = nodeMap.get(animation.fromNode);
    const toNode = nodeMap.get(animation.toNode);
    
    if (!fromNode || !toNode) return;

    // Calculate geodesic path
    const geodesicPoints = calculateGeodesic(
      fromNode.coordinate,
      toNode.coordinate,
      30
    );

    if (geodesicPoints.length < 2) return;

    // Find position along the path based on progress
    const totalPoints = geodesicPoints.length - 1;
    const exactIndex = animation.progress * totalPoints;
    const index = Math.floor(exactIndex);
    const fraction = exactIndex - index;

    // Interpolate between points
    let packetPos;
    if (index >= totalPoints) {
      packetPos = geodesicPoints[totalPoints];
    } else {
      const p1 = geodesicPoints[index];
      const p2 = geodesicPoints[index + 1];
      packetPos = {
        x: p1.x + fraction * (p2.x - p1.x),
        y: p1.y + fraction * (p2.y - p1.y),
      };
    }

    // Convert to canvas coordinates
    const canvasPos = toCanvasCoords(packetPos, width, height, scale, offset.x, offset.y);

    // Get color based on routing mode
    const color = MODE_COLORS[animation.mode];

    // Draw packet trail (fading line along path)
    const trailLength = Math.min(index + 1, 10);
    const startIndex = Math.max(0, index - trailLength + 1);
    
    ctx.beginPath();
    const trailPoints = geodesicPoints.slice(startIndex, index + 2).map(p =>
      toCanvasCoords(p, width, height, scale, offset.x, offset.y)
    );
    
    if (trailPoints.length > 1) {
      // Create gradient for trail
      const gradient = ctx.createLinearGradient(
        trailPoints[0].x, trailPoints[0].y,
        trailPoints[trailPoints.length - 1].x, trailPoints[trailPoints.length - 1].y
      );
      gradient.addColorStop(0, 'transparent');
      gradient.addColorStop(1, color);

      ctx.beginPath();
      ctx.moveTo(trailPoints[0].x, trailPoints[0].y);
      for (let i = 1; i < trailPoints.length; i++) {
        ctx.lineTo(trailPoints[i].x, trailPoints[i].y);
      }
      ctx.strokeStyle = gradient;
      ctx.lineWidth = 3;
      ctx.stroke();
    }

    // Draw packet glow
    const glowGradient = ctx.createRadialGradient(
      canvasPos.x, canvasPos.y, 0,
      canvasPos.x, canvasPos.y, 15
    );
    glowGradient.addColorStop(0, color);
    glowGradient.addColorStop(0.5, `${color}66`);
    glowGradient.addColorStop(1, 'transparent');

    ctx.beginPath();
    ctx.arc(canvasPos.x, canvasPos.y, 15, 0, 2 * Math.PI);
    ctx.fillStyle = glowGradient;
    ctx.fill();

    // Draw packet core
    ctx.beginPath();
    ctx.arc(canvasPos.x, canvasPos.y, 6, 0, 2 * Math.PI);
    ctx.fillStyle = color;
    ctx.fill();
    ctx.strokeStyle = '#fff';
    ctx.lineWidth = 2;
    ctx.stroke();

    // Draw mode indicator on packet
    ctx.fillStyle = '#fff';
    ctx.font = 'bold 8px monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    const modeChar = animation.mode[0].toUpperCase();
    ctx.fillText(modeChar, canvasPos.x, canvasPos.y);
  });
}

/**
 * Draw routing mode indicator in the top-left corner
 */
function drawModeIndicator(
  ctx: CanvasRenderingContext2D,
  mode: RoutingMode,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  _width: number
) {
  const color = MODE_COLORS[mode];
  const name = MODE_NAMES[mode];

  // Background
  ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
  ctx.fillRect(10, 10, 100, 35);
  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.strokeRect(10, 10, 100, 35);

  // Mode indicator dot
  ctx.beginPath();
  ctx.arc(25, 27, 8, 0, 2 * Math.PI);
  ctx.fillStyle = color;
  ctx.fill();

  // Mode name
  ctx.fillStyle = '#fff';
  ctx.font = 'bold 12px monospace';
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  ctx.fillText(name, 40, 27);

  // Pulsing animation effect (using time-based opacity)
  const pulse = Math.sin(Date.now() / 200) * 0.3 + 0.7;
  ctx.beginPath();
  ctx.arc(25, 27, 10, 0, 2 * Math.PI);
  ctx.strokeStyle = `${color}${Math.floor(pulse * 255).toString(16).padStart(2, '0')}`;
  ctx.lineWidth = 2;
  ctx.stroke();
}
