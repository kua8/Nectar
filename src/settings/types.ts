import type { ComponentType, SVGProps } from "react";

export interface WidgetConfig {
  left: string[];
  right: string[];
}

export interface MonitorInfo {
  id: string;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
  is_primary: boolean;
}

export type MonitorMode = "primary" | "all" | "specific";

export type SettingsTab = "general" | "appearance" | "notch" | "dock" | "overlays" | "about";

export interface SettingRowProps {
  icon: ComponentType<SVGProps<SVGSVGElement> & { size?: number; strokeWidth?: number }>;
  label: string;
  desc?: string;
  action?: boolean;
  danger?: boolean;
  divider?: boolean;
  onClick?: () => void;
  children?: React.ReactNode;
}
