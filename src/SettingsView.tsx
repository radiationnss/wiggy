import { type FormEvent } from 'react';
import type { CodexConnection, CodexModel, Settings, Status, PopupPosition } from './types';

const positions: { value: PopupPosition; label: string }[] = [
  { value: 'near_cursor', label: 'Near cursor' },
  { value: 'top_left', label: 'Top left' },
  { value: 'top_right', label: 'Top right' },
  { value: 'bottom_left', label: 'Bottom left' },
  { value: 'bottom_right', label: 'Bottom right' },
];

type Props = {
  settings: Settings | null;
  status: Status | null;
  codexConnection: CodexConnection | null;
  models: CodexModel[];
  modelsLoading: boolean;
  modelsError: string;
  connectionLoading: boolean;
  native: boolean;
  error: string;
  saving: boolean;
  notice: string;
  onChange: (settings: Settings) => void;
  onSave: (event: FormEvent) => void;
  onRetry: () => void;
  onRefreshConnection: () => void;
};

export function SettingsView({ settings, status, codexConnection, models, modelsLoading, modelsError, connectionLoading, native, error, saving, notice, onChange, onSave, onRetry, onRefreshConnection }: Props) {
  return <form className="settings-form flex min-h-0 flex-1 flex-col" onSubmit={onSave}>
    <div className="settings-scroll min-h-0 flex-1 overflow-y-auto">
      {error && <div className="banner error" role="alert">{error}<button className="secondary" type="button" onClick={onRetry}>Retry loading</button></div>}
      <section className="settings-section ai-connection" aria-label="ChatGPT connection">
        <div className="setting-row">
          <span title="Wiggy uses the ChatGPT login already connected in Codex; no separate sign-in is needed.">ChatGPT account</span>
          <div className="connection-actions">
            {codexConnection && <span className={`connection-state ${codexConnection.state === 'connected' ? 'is-connected' : ''}`} role="status" title={codexConnection.message}>{codexConnection.state === 'connected' ? 'Connected' : codexConnection.state === 'not_running' ? 'Codex is offline' : codexConnection.state === 'not_signed_in' ? 'Not signed in' : 'Unavailable'}</span>}
            <button className="secondary" type="button" disabled={!native || connectionLoading} onClick={onRefreshConnection}>{connectionLoading ? 'Checking…' : 'Refresh'}</button>
          </div>
        </div>
      </section>
      {settings ? <>
        <fieldset className="settings-section model-fieldset" disabled={!native || saving} aria-label="GPT model">
          <div className="setting-row"><label htmlFor="gpt-model" title="Choose a model from the models available to your Codex login. The Codex default is used when no override is selected.">GPT model</label>
            <select id="gpt-model" value={settings.model ?? ''} title={modelsError || 'Choose the model Wiggy uses for new replies.'} onChange={event => onChange({ ...settings, model: event.target.value || null })}>
              <option value="">Codex default{models.find(model => model.isDefault) ? ` · ${models.find(model => model.isDefault)?.displayName}` : ''}</option>
              {models.map(model => <option value={model.id} key={model.id}>{model.displayName}{model.isDefault ? ' · default' : ''}</option>)}
            </select>
          </div>
          {modelsLoading && <span className="sr-only" role="status">Loading available models</span>}
        </fieldset>
        <fieldset className="position-fieldset" disabled={!native || !status?.position_supported || saving}>
          <legend title="Choose where Wiggy opens on screen.">Popup position</legend>
          <div className="position-picker">
          <div className="position-window" role="radiogroup" aria-label="Popup position">
            <div className="position-window-chrome" aria-hidden="true"><span /><span /><span /></div>
            {positions.map(position => <label key={position.value} title={position.label} className={`position-option pin-${position.value} ${settings.popup_position === position.value ? 'selected' : ''}`}>
              <input type="radio" name="popup_position" value={position.value} checked={settings.popup_position === position.value} onChange={() => onChange({ ...settings, popup_position: position.value })} />
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={position.value === 'near_cursor' ? 'M5 4l14 6-6 3-3 6z' : 'M6 16V6h10'} /></svg>
              <span className="sr-only">{position.label}</span>
            </label>)}
          </div>
          <p className="position-selection" role="status">{positions.find(position => position.value === settings.popup_position)?.label}</p>
          </div>
        </fieldset>
        <fieldset className="settings-section" disabled={!native || saving} aria-label="Mouse trigger">
          <div className="setting-row"><label htmlFor="mouse-enabled" title="Open Wiggy when the mouse makes a quick wiggle gesture.">Mouse wiggle</label><div className="flex items-center gap-2">{status && !status.mouse_supported && <span className="trigger-state" title={status.error || 'Mouse tracking is unavailable'}>Unavailable</span>}<label className="switch"><input id="mouse-enabled" type="checkbox" checked={settings.mouse_enabled} onChange={event => onChange({ ...settings, mouse_enabled: event.target.checked })} /><span aria-hidden="true" /></label></div></div>
          <div className="setting-row"><label htmlFor="sensitivity" title="Set how much movement counts as a wiggle.">Sensitivity</label><div className="range-control"><input id="sensitivity" type="range" min="0.5" max="2" step="0.1" value={settings.sensitivity} onChange={event => onChange({ ...settings, sensitivity: Number(event.target.value) })} /><output htmlFor="sensitivity">{settings.sensitivity.toFixed(1)}×</output></div></div>
          <div className="setting-row"><label htmlFor="cooldown" title="Set how long Wiggy waits before the next mouse-triggered opening.">Cooldown</label><div className="cooldown-control"><input id="cooldown" aria-label="Cooldown (milliseconds)" type="number" min="500" max="30000" step="100" required value={settings.cooldown_ms} onChange={event => onChange({ ...settings, cooldown_ms: Number(event.target.value) })} /><span>ms</span></div></div>
        </fieldset>
      </> : <p className="scope">{error ? 'Settings could not be loaded. Retry to continue.' : native ? 'Loading settings…' : 'Settings require the native desktop app.'}</p>}
      <details className="integration"><summary>Advanced</summary>
        {status && <>
        <p className="scope">{status.platform} {status.version}</p>
        {status.error && <p className="scope">{status.error}</p>}
        <p className="scope">Keyboard shortcut: Super + Shift + Space. Set your binary path below.</p><pre>{status.instructions}</pre>
        </>}
        <a className="credit" href="https://designlounge.vercel.app">Designed using Design Lounge</a>
      </details>
    </div>
    <div className="settings-actions flex shrink-0 items-center justify-between">
      <div role="status" className="save-notice">{notice}</div>
      <button className="primary" aria-label="Save settings" disabled={!native || !settings || saving}>{saving ? 'Saving…' : 'Save'}</button>
    </div>
  </form>;
}
