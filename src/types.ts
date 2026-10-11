export type PopupPosition = 'near_cursor' | 'top_left' | 'top_right' | 'bottom_left' | 'bottom_right';
export type Settings = { mouse_enabled: boolean; sensitivity: number; cooldown_ms: number; popup_position: PopupPosition; model: string | null };
export type Status = { platform: string; version: string | null; mouse_supported: boolean; position_supported: boolean; error: string | null; instructions: string };
export type CodexConnection = { state: 'connected' | 'not_running' | 'not_signed_in' | 'unavailable'; message: string };
export type CodexModel = { id: string; displayName: string; inputModalities: string[]; isDefault: boolean };
export type Screenshot = { data_url: string; width: number; height: number; bytes: number; label: string };
export type CaptureTarget = 'screen' | 'region' | 'window';
