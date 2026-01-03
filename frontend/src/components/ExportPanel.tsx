/**
 * Export Panel Component
 * Provides UI for exporting the visualization to SVG/PNG formats
 * for use in academic papers
 */

import { useState, useCallback } from 'react';
import type { NetworkTopology, RenderOptions, RoutingMode } from '../types';
import { exportToSVG, exportToPNG, generateSVG } from '../utils/exportFigure';

interface ExportPanelProps {
  topology: NetworkTopology;
  options: Partial<RenderOptions>;
  selectedNode?: string;
  activeMode?: RoutingMode | null;
}

type ExportFormat = 'svg' | 'png';
type ExportSize = 'small' | 'medium' | 'large' | 'custom';

const SIZE_PRESETS: Record<ExportSize, { width: number; height: number; label: string }> = {
  small: { width: 400, height: 400, label: 'Small (400×400)' },
  medium: { width: 800, height: 800, label: 'Medium (800×800)' },
  large: { width: 1200, height: 1200, label: 'Large (1200×1200)' },
  custom: { width: 800, height: 800, label: 'Custom' },
};

export function ExportPanel({
  topology,
  options,
  selectedNode,
  activeMode,
}: ExportPanelProps) {
  const [format, setFormat] = useState<ExportFormat>('svg');
  const [sizePreset, setSizePreset] = useState<ExportSize>('medium');
  const [customWidth, setCustomWidth] = useState(800);
  const [customHeight, setCustomHeight] = useState(800);
  const [scaleFactor, setScaleFactor] = useState(2);
  const [includeTitle, setIncludeTitle] = useState(true);
  const [customTitle, setCustomTitle] = useState('DRFE-R Network Topology');
  const [includeTimestamp, setIncludeTimestamp] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [previewSVG, setPreviewSVG] = useState<string | null>(null);

  const getExportDimensions = useCallback(() => {
    if (sizePreset === 'custom') {
      return { width: customWidth, height: customHeight };
    }
    return SIZE_PRESETS[sizePreset];
  }, [sizePreset, customWidth, customHeight]);

  const handleExport = useCallback(async () => {
    setIsExporting(true);
    try {
      const dimensions = getExportDimensions();
      const exportOptions = {
        ...options,
        width: dimensions.width,
        height: dimensions.height,
        scaleFactor,
        title: includeTitle ? customTitle : undefined,
        includeTimestamp,
        selectedNode,
        activeMode,
      };

      const timestamp = new Date().toISOString().slice(0, 10);
      const filename = `drfe-r-topology-${timestamp}`;

      if (format === 'svg') {
        exportToSVG(topology, exportOptions, `${filename}.svg`);
      } else {
        await exportToPNG(topology, exportOptions, `${filename}.png`);
      }
    } catch (error) {
      console.error('Export failed:', error);
      alert('Export failed. Please try again.');
    } finally {
      setIsExporting(false);
    }
  }, [
    format,
    topology,
    options,
    getExportDimensions,
    scaleFactor,
    includeTitle,
    customTitle,
    includeTimestamp,
    selectedNode,
    activeMode,
  ]);

  const handlePreview = useCallback(() => {
    const dimensions = getExportDimensions();
    const exportOptions = {
      ...options,
      width: Math.min(dimensions.width, 400), // Limit preview size
      height: Math.min(dimensions.height, 400),
      title: includeTitle ? customTitle : undefined,
      includeTimestamp,
      selectedNode,
      activeMode,
    };

    const svg = generateSVG(topology, exportOptions);
    setPreviewSVG(svg);
  }, [
    topology,
    options,
    getExportDimensions,
    includeTitle,
    customTitle,
    includeTimestamp,
    selectedNode,
    activeMode,
  ]);

  const closePreview = useCallback(() => {
    setPreviewSVG(null);
  }, []);

  return (
    <section className="export-panel">
      <h2>📤 Export Figure</h2>
      <p className="export-description">
        Export high-quality figures for academic papers
      </p>

      <div className="export-options">
        {/* Format Selection */}
        <div className="export-option-group">
          <label>Format:</label>
          <div className="export-format-buttons">
            <button
              className={`format-btn ${format === 'svg' ? 'active' : ''}`}
              onClick={() => setFormat('svg')}
            >
              SVG
            </button>
            <button
              className={`format-btn ${format === 'png' ? 'active' : ''}`}
              onClick={() => setFormat('png')}
            >
              PNG
            </button>
          </div>
          <small className="format-hint">
            {format === 'svg' 
              ? 'Vector format - scalable, editable' 
              : 'Raster format - high resolution'}
          </small>
        </div>

        {/* Size Selection */}
        <div className="export-option-group">
          <label>Size:</label>
          <select 
            value={sizePreset} 
            onChange={(e) => setSizePreset(e.target.value as ExportSize)}
            className="export-select"
          >
            {Object.entries(SIZE_PRESETS).map(([key, { label }]) => (
              <option key={key} value={key}>{label}</option>
            ))}
          </select>
        </div>

        {/* Custom Size Inputs */}
        {sizePreset === 'custom' && (
          <div className="export-option-group custom-size">
            <div className="size-input">
              <label>Width:</label>
              <input
                type="number"
                value={customWidth}
                onChange={(e) => setCustomWidth(Math.max(100, parseInt(e.target.value) || 100))}
                min={100}
                max={4000}
              />
            </div>
            <div className="size-input">
              <label>Height:</label>
              <input
                type="number"
                value={customHeight}
                onChange={(e) => setCustomHeight(Math.max(100, parseInt(e.target.value) || 100))}
                min={100}
                max={4000}
              />
            </div>
          </div>
        )}

        {/* Scale Factor for PNG */}
        {format === 'png' && (
          <div className="export-option-group">
            <label>Scale Factor (DPI):</label>
            <select 
              value={scaleFactor} 
              onChange={(e) => setScaleFactor(parseInt(e.target.value))}
              className="export-select"
            >
              <option value={1}>1x (72 DPI)</option>
              <option value={2}>2x (144 DPI) - Recommended</option>
              <option value={3}>3x (216 DPI)</option>
              <option value={4}>4x (288 DPI) - Print Quality</option>
            </select>
          </div>
        )}

        {/* Title Options */}
        <div className="export-option-group">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={includeTitle}
              onChange={(e) => setIncludeTitle(e.target.checked)}
            />
            Include Title
          </label>
          {includeTitle && (
            <input
              type="text"
              value={customTitle}
              onChange={(e) => setCustomTitle(e.target.value)}
              placeholder="Figure title"
              className="export-input"
            />
          )}
        </div>

        {/* Timestamp Option */}
        <div className="export-option-group">
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={includeTimestamp}
              onChange={(e) => setIncludeTimestamp(e.target.checked)}
            />
            Include Timestamp
          </label>
        </div>
      </div>

      {/* Export Actions */}
      <div className="export-actions">
        <button 
          className="preview-btn" 
          onClick={handlePreview}
          disabled={isExporting}
        >
          👁️ Preview
        </button>
        <button 
          className="export-btn" 
          onClick={handleExport}
          disabled={isExporting}
        >
          {isExporting ? '⏳ Exporting...' : `📥 Export ${format.toUpperCase()}`}
        </button>
      </div>

      {/* Preview Modal */}
      {previewSVG && (
        <div className="export-preview-modal" onClick={closePreview}>
          <div className="export-preview-content" onClick={(e) => e.stopPropagation()}>
            <div className="preview-header">
              <h3>Export Preview</h3>
              <button className="close-btn" onClick={closePreview}>×</button>
            </div>
            <div 
              className="preview-svg"
              dangerouslySetInnerHTML={{ __html: previewSVG }}
            />
            <div className="preview-footer">
              <small>Click outside or press × to close</small>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
