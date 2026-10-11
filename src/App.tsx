import { useEffect, useRef, useState, type FormEvent } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { SettingsView } from './SettingsView';
import type { CaptureTarget, CodexConnection, CodexModel, Screenshot, Settings, Status } from './types';

type Attachment = Screenshot & { id: number };
type Message = { id: string; requestId: string; role: 'user' | 'assistant'; text: string; images: Attachment[]; status?: 'thinking' | 'completed' | 'interrupted' | 'failed' };
type ChatDelta = { requestId: string; delta: string };
type ChatFinished = { requestId: string; status: 'completed' | 'interrupted' | 'failed'; error: string | null };

function Icon({ name }: { name: 'arrow' | 'back' | 'settings' | 'minus' | 'trash' | 'area' | 'screen' | 'window' | 'chevron' }) {
  const paths = {
    arrow: 'M12 19V5M6 11l6-6 6 6',
    back: 'M19 12H5M11 6l-6 6 6 6',
    settings: 'M10 3h4l.5 2.5 2 1.2 2.4-.8 2 3.4-1.9 1.7v2l1.9 1.7-2 3.4-2.4-.8-2 1.2L14 21h-4l-.5-2.5-2-1.2-2.4.8-2-3.4L5 13v-2L3.1 9.3l2-3.4 2.4.8 2-1.2z',
    minus: 'M5 12h14',
    trash: 'M5 7h14M9 7V5h6v2M8 7l1 13h6l1-13',
    area: 'M8 3H3v5M16 3h5v5M21 16v5h-5M8 21H3v-5',
    screen: 'M3 4h18v12H3zM8 21h8M12 16v5',
    window: 'M3 4h18v16H3zM3 9h18M7 6.5h.1M10 6.5h.1',
    chevron: 'M7 10l5 5 5-5',
  };
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{name === 'settings' && <circle cx="12" cy="12" r="3" />}<path d={paths[name]} /></svg>;
}

export function App() {
  const [view, setView] = useState<'palette' | 'settings'>('palette');
  const [text, setText] = useState('');
  const [messages, setMessages] = useState<Message[]>([]);
  const [chatBusy, setChatBusy] = useState(false);
  const [chatError, setChatError] = useState('');
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [captureMenu, setCaptureMenu] = useState(false);
  const [capturing, setCapturing] = useState(false);
  const [captureError, setCaptureError] = useState('');
  const [preview, setPreview] = useState<Attachment | null>(null);
  const captureBusy = useRef(false);
  const attachmentId = useRef(0);
  const requestId = useRef(0);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [codexConnection, setCodexConnection] = useState<CodexConnection | null>(null);
  const [models, setModels] = useState<CodexModel[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsError, setModelsError] = useState('');
  const [connectionLoading, setConnectionLoading] = useState(false);
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState('');
  const [settingsError, setSettingsError] = useState('');
  const [nativeReady, setNativeReady] = useState(false);
  const [openRevision, setOpenRevision] = useState(0);
  const input = useRef<HTMLTextAreaElement>(null);
  const log = useRef<HTMLDivElement>(null);
  const captureControl = useRef<HTMLDivElement>(null);
  const viewRevision = useRef(0);
  const native = isTauri();
  const hasMessages = messages.length > 0;
  const expanded = hasMessages || attachments.length > 0 || Boolean(captureError) || Boolean(chatError);
  const selectedModel = models.find(model => model.id === settings?.model) ?? models.find(model => model.isDefault);
  const selectedModelSupportsImages = selectedModel?.inputModalities.includes('image') ?? false;

  function changeView(next: 'palette' | 'settings') {
    viewRevision.current += 1;
    setView(next);
    setNotice('');
  }

  async function loadSettings() {
    setSettingsError('');
    try { setSettings(await invoke<Settings>('get_settings')); }
    catch (failure) { setSettingsError(String(failure)); }
  }

  async function refreshCodexConnection() {
    setConnectionLoading(true);
    await Promise.all([loadConnection(), loadModels()]);
    setConnectionLoading(false);
  }

  async function loadConnection() {
    try { setCodexConnection(await invoke<CodexConnection>('get_codex_connection')); }
    catch { setCodexConnection({ state: 'unavailable', message: 'Could not read the Codex connection. Check that Codex is running, then refresh.' }); }
  }

  async function loadModels() {
    setModelsLoading(true);
    setModelsError('');
    try { setModels(await invoke<CodexModel[]>('get_codex_models')); }
    catch (failure) { setModelsError(String(failure)); setModels([]); }
    finally { setModelsLoading(false); }
  }

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const initialRevision = viewRevision.current;
    const subscriptions: (() => void)[] = [];
    invoke<Settings>('get_settings').then(configuration => { if (!disposed) setSettings(configuration); }).catch(failure => { if (!disposed) setSettingsError(String(failure)); });
    invoke<Status>('get_status').then(capability => { if (!disposed) setStatus(capability); }).catch(failure => { if (!disposed) setError(`Desktop diagnostics: ${String(failure)}`); });
    invoke<CodexConnection>('get_codex_connection').then(connection => { if (!disposed) setCodexConnection(connection); }).catch(() => { if (!disposed) setCodexConnection({ state: 'unavailable', message: 'Could not read the Codex connection. Check that Codex is running, then refresh.' }); });
    setModelsLoading(true);
    invoke<CodexModel[]>('get_codex_models').then(available => { if (!disposed) setModels(available); }).catch(failure => { if (!disposed) setModelsError(String(failure)); }).finally(() => { if (!disposed) setModelsLoading(false); });
    const subscribe = async () => {
      for (const event of ['popup-opened', 'settings-opened']) {
        const unlisten = await listen(event, () => {
          changeView(event === 'settings-opened' ? 'settings' : 'palette');
          setOpenRevision(previous => previous + 1);
          requestAnimationFrame(() => input.current?.focus());
        });
        if (disposed) unlisten(); else subscriptions.push(unlisten);
      }
      const unlistenStatus = await listen('trigger-status-changed', () => {
        invoke<Status>('get_status').then(capability => { if (!disposed) setStatus(capability); }).catch(failure => { if (!disposed) setError(`Desktop diagnostics: ${String(failure)}`); });
      });
      if (disposed) unlistenStatus(); else subscriptions.push(unlistenStatus);
      const unlistenDelta = await listen<ChatDelta>('chat-delta', event => {
        setMessages(previous => previous.map(message => message.role === 'assistant' && message.requestId === event.payload.requestId
          ? { ...message, text: message.text + event.payload.delta }
          : message));
      });
      if (disposed) unlistenDelta(); else subscriptions.push(unlistenDelta);
      const unlistenFinish = await listen<ChatFinished>('chat-finished', event => {
        const { requestId: finishedId, status: finishedStatus, error: failure } = event.payload;
        setChatBusy(false);
        setMessages(previous => previous.map(message => message.role === 'assistant' && message.requestId === finishedId
          ? { ...message, status: finishedStatus, text: failure && !message.text ? failure : message.text }
          : message));
        setChatError(failure ?? '');
      });
      if (disposed) unlistenFinish(); else subscriptions.push(unlistenFinish);
      if (disposed) return;
      const settingsOpen = await invoke<boolean>('frontend_ready');
      if (!disposed) {
        if (viewRevision.current === initialRevision) setView(settingsOpen ? 'settings' : 'palette');
        setNativeReady(true);
      }
    };
    subscribe().catch(failure => { if (!disposed) setError(String(failure)); });
    return () => { disposed = true; subscriptions.forEach(unlisten => unlisten()); };
  }, [native]);

  useEffect(() => {
    if (!nativeReady) return;
    invoke('fit_popup', { layout: view === 'settings' ? 'settings' : expanded ? 'chat' : 'compact' }).catch(failure => setError(`Window sizing: ${String(failure)}`));
  }, [nativeReady, view, expanded, openRevision]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        if (preview) { setPreview(null); return; }
        if (captureMenu) { setCaptureMenu(false); input.current?.focus(); return; }
        if (native) invoke('hide_popup').catch(failure => setError(String(failure)));
        else setView('palette');
      }
    };
    window.addEventListener('keydown', onKey);
    if (view === 'palette' && !captureMenu && !preview) input.current?.focus();
    return () => window.removeEventListener('keydown', onKey);
  }, [native, view, preview, captureMenu]);

  useEffect(() => { if (log.current) log.current.scrollTop = log.current.scrollHeight; }, [messages]);

  useEffect(() => {
    if (!captureMenu) return;
    const dismiss = (event: PointerEvent) => {
      if (event.target instanceof Node && !captureControl.current?.contains(event.target)) setCaptureMenu(false);
    };
    document.addEventListener('pointerdown', dismiss);
    return () => document.removeEventListener('pointerdown', dismiss);
  }, [captureMenu]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const message = text.trim();
    if (capturing || chatBusy || (!message && !attachments.length)) return;
    if (!native || codexConnection?.state !== 'connected') {
      setChatError('Connect the existing Codex ChatGPT login in Settings before sending.');
      return;
    }
    if (attachments.length && !selectedModelSupportsImages) {
      setChatError('The selected model does not support screenshots. Choose an image-capable GPT model in Settings.');
      return;
    }
    const id = `wiggy-${Date.now()}-${++requestId.current}`;
    const sentImages = attachments;
    setChatError('');
    setMessages(previous => [...previous.slice(-48),
      { id: `user-${id}`, requestId: id, role: 'user', text: message, images: sentImages },
      { id: `assistant-${id}`, requestId: id, role: 'assistant', text: '', images: [], status: 'thinking' },
    ]);
    setChatBusy(true);
    setCaptureMenu(false);
    setText('');
    setAttachments([]);
    try {
      await invoke('send_chat', { requestId: id, text: message, images: sentImages.map(image => ({ dataUrl: image.data_url })) });
    } catch (failure) {
      setChatBusy(false);
      setMessages(previous => previous.map(item => item.role === 'assistant' && item.requestId === id
        ? { ...item, status: 'failed', text: String(failure) }
        : item));
    }
  }

  async function clearChat() {
    if (chatBusy) return;
    if (native) {
      try { await invoke('clear_chat'); }
      catch (failure) { setChatError(String(failure)); return; }
    }
    setMessages([]);
    setChatError('');
  }

  function stopChat() {
    invoke('stop_chat').catch(failure => setChatError(String(failure)));
  }

  async function capture(target: CaptureTarget) {
    if (captureBusy.current || attachments.length >= 3) return;
    captureBusy.current = true;
    setCapturing(true);
    setCaptureMenu(false);
    setCaptureError('');
    try {
      const image = await invoke<Screenshot | null>('capture_screen', { target });
      if (!image) return;
      const retainedBytes = [...messages.flatMap(message => message.images), ...attachments].reduce((total, image) => total + image.bytes, 0);
      if (retainedBytes + image.bytes > 24 * 1024 * 1024) {
        setCaptureError('Screenshot session is full. Clear chat or remove an attachment.');
        return;
      }
      const attachment = { ...image, id: ++attachmentId.current };
      setAttachments(previous => [...previous, attachment]);
    } catch (failure) {
      setCaptureError(`Screen capture: ${String(failure)}`);
    } finally {
      captureBusy.current = false;
      setCapturing(false);
      requestAnimationFrame(() => input.current?.focus());
    }
  }

  function hide() {
    invoke('hide_popup').catch(failure => setError(String(failure)));
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    if (!settings) return;
    setSaving(true); setSettingsError(''); setNotice('');
    try {
      setSettings(await invoke<Settings>('save_settings', { settings }));
      setNotice('Settings saved.');
    } catch (failure) { setSettingsError(String(failure)); }
    finally { setSaving(false); }
  }

  return (
    <main className={`palette flex h-screen flex-col overflow-hidden ${view === 'settings' ? 'settings-mode' : expanded ? 'chat-mode' : 'compact-mode'}`} aria-label="Wiggy command palette">
      <header className="desktop-bar flex shrink-0 items-center justify-between">
        <div className="context-label flex items-center gap-2"><h1>{view === 'settings' ? 'Settings' : 'Wiggy'}</h1>{view === 'palette' && <span className={`local-status ${codexConnection?.state === 'connected' ? 'connected' : ''}`} title={codexConnection?.message ?? 'Checking the existing Codex login.'}>{codexConnection?.state === 'connected' ? 'ChatGPT connected' : 'Local only'}</span>}</div>
        <div className="header-actions flex items-center gap-1">
          {view === 'palette' && hasMessages && <button className="icon-button clear-chat" aria-label="Clear chat" disabled={chatBusy} onClick={clearChat}><Icon name="trash" /></button>}
          <button className="icon-button" aria-label={view === 'palette' ? 'Settings' : 'Back to chat'} onClick={() => changeView(view === 'palette' ? 'settings' : 'palette')}><Icon name={view === 'palette' ? 'settings' : 'back'} /></button>
          <button className="icon-button" aria-label="Hide Wiggy" disabled={!native} onClick={hide}><Icon name="minus" /></button>
        </div>
      </header>
      {!native && <p className="banner">Browser preview. Desktop triggers and settings require the native app.</p>}
      {error && <p className="banner error" role="alert">{error}</p>}
      {view === 'palette' ? <>
        <div ref={log} className="conversation min-h-0 flex-1 overflow-y-auto" role="log" aria-label="Conversation" aria-live="polite">
          {hasMessages && <>
            {messages.map(message => <article className={`message-row ${message.role === 'assistant' ? 'assistant-row' : ''}`} key={message.id} aria-label={message.role === 'assistant' ? 'Wiggy reply' : 'Your message'}><div className={`message ${message.role === 'assistant' ? 'assistant-message' : ''}`} data-status={message.status}>
              {message.role === 'assistant' && <div className="message-meta">Wiggy</div>}
              {message.images.map(image => <button type="button" className="chat-image" key={image.id} aria-label={`View ${image.label.toLowerCase()} screenshot`} onClick={() => setPreview(image)}><img src={image.data_url} alt={`${image.label} screenshot`} /></button>)}
              {message.text && <p>{message.text}</p>}
              {message.role === 'assistant' && message.status === 'thinking' && !message.text && <p className="scope">Thinking…</p>}
            </div></article>)}
          </>}
        </div>
        <form onSubmit={submit} className="composer shrink-0">
          {captureError && <p className="banner error" role="alert">{captureError}</p>}
          {chatError && <p className="banner error" role="alert">{chatError}</p>}
          {attachments.length > 0 && <div className="attachments" aria-label="Screenshot attachments">
            {attachments.map(image => <div className="attachment" key={image.id}>
              <button type="button" className="attachment-preview" aria-label={`Preview ${image.label.toLowerCase()} screenshot`} onClick={() => setPreview(image)}><img src={image.data_url} alt={`${image.label} screenshot`} /></button>
              <button type="button" className="attachment-remove" aria-label={`Remove ${image.label.toLowerCase()} screenshot`} onClick={() => setAttachments(previous => previous.filter(attachment => attachment.id !== image.id))}>×</button>
            </div>)}
          </div>}
          <div className="composer-input flex items-center">
          <label htmlFor="prompt" className="sr-only">Your message</label>
          <textarea ref={input} id="prompt" rows={2} value={text} onChange={event => setText(event.target.value)} onKeyDown={event => {
            if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); }
          }} placeholder={native ? 'Message ChatGPT' : 'Message Wiggy'} maxLength={8000} autoComplete="off" spellCheck={false} />
          {chatBusy ? <button className="stop-button" type="button" onClick={stopChat}>Stop</button> : <button className="send-button" aria-label="Send message to ChatGPT" title={codexConnection?.state === 'connected' ? (attachments.length ? 'Send message and screenshot to ChatGPT' : 'Send text to ChatGPT') : 'Connect the existing Codex login in Settings'} disabled={capturing || (!text.trim() && !attachments.length) || !native || codexConnection?.state !== 'connected'} type="submit"><Icon name="arrow" /></button>}
          </div>
          <div className="suggestions flex flex-wrap items-center gap-1">
            <button type="button" aria-pressed="true" onClick={() => input.current?.focus()}>Ask anything</button>
            <div className="capture-control" ref={captureControl} onBlur={event => {
              if (!event.currentTarget.contains(event.relatedTarget)) setCaptureMenu(false);
            }}>
              <button type="button" className="capture-toggle" aria-expanded={captureMenu} aria-controls="capture-choices" disabled={!native || status?.platform !== 'Hyprland' || capturing || attachments.length >= 3 || !selectedModelSupportsImages} title={!selectedModelSupportsImages ? 'Choose a model that supports images in Settings' : attachments.length >= 3 ? 'Up to three screenshots per message' : 'Capture locally; the screenshot is sent only when you press Send'} onClick={() => setCaptureMenu(previous => !previous)}>{capturing ? 'Capturing…' : 'Use screen'}<Icon name="chevron" /></button>
              {captureMenu && <div className="capture-choices" id="capture-choices" role="group" aria-label="Capture screenshot" onKeyDown={event => {
                if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
                event.preventDefault();
                const options = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('button'));
                const selected = options.indexOf(document.activeElement as HTMLButtonElement);
                const next = event.key === 'Home' ? 0 : event.key === 'End' ? options.length - 1 : (selected + (event.key === 'ArrowUp' ? -1 : 1) + options.length) % options.length;
                options[next]?.focus();
              }}>
                <button type="button" onClick={() => capture('region')}><Icon name="area" />Select area</button>
                <button type="button" onClick={() => capture('screen')} title="Capture all connected displays"><Icon name="screen" />Entire screen</button>
                <button type="button" onClick={() => capture('window')} title="Select a visible window. Overlapping content may appear."><Icon name="window" />Select window</button>
              </div>}
            </div>
          </div>
        </form>
      </> : view === 'settings' ? <SettingsView settings={settings} status={status} codexConnection={codexConnection} models={models} modelsLoading={modelsLoading} modelsError={modelsError} connectionLoading={connectionLoading} native={native} error={settingsError} saving={saving} notice={notice} onChange={setSettings} onSave={save} onRetry={loadSettings} onRefreshConnection={refreshCodexConnection} /> : null}
      {preview && <div className="image-preview" role="dialog" aria-modal="true" aria-label="Screenshot preview" onKeyDown={event => {
        if (event.key === 'Tab') { event.preventDefault(); event.currentTarget.querySelector<HTMLButtonElement>('button')?.focus(); }
      }}>
        <button type="button" className="secondary" autoFocus onClick={() => { setPreview(null); input.current?.focus(); }}>Close preview</button>
        <img src={preview.data_url} alt={`${preview.label} screenshot preview`} />
        <span>{preview.width} × {preview.height} · Session only</span>
      </div>}
    </main>
  );
}
