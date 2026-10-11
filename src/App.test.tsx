import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from './App';

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
const listeners = vi.hoisted(() => new Map<string, Set<(event: { payload: unknown }) => void>>());
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke, isTauri: () => true }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));

const configuration = { mouse_enabled: true, sensitivity: 1, cooldown_ms: 2500, popup_position: 'near_cursor', model: null };
const models = [{ id: 'gpt-6.1-sol', displayName: 'GPT-6.1-Sol', inputModalities: ['text', 'image'], isDefault: true }];
const capability = { platform: 'Hyprland', version: '0.56.2', mouse_supported: false, position_supported: true, error: null, instructions: 'manual bind' };

beforeEach(() => {
  listeners.clear();
  mocks.invoke.mockReset();
  mocks.listen.mockReset().mockImplementation(async (event: string, callback: (event: { payload: unknown }) => void) => {
    const handlers = listeners.get(event) ?? new Set();
    handlers.add(callback);
    listeners.set(event, handlers);
    return () => handlers.delete(callback);
  });
  mocks.invoke.mockImplementation(async (command: string, argumentsValue?: { settings?: typeof configuration }) => {
    if (command === 'get_settings') return { ...configuration };
    if (command === 'get_status') return { ...capability };
    if (command === 'get_codex_connection') return { state: 'connected', message: 'Using your existing Codex ChatGPT login.' };
    if (command === 'get_codex_models') return [...models];
    if (command === 'save_settings') return argumentsValue?.settings;
    if (command === 'send_chat') {
      const requestId = (argumentsValue as { requestId?: string } | undefined)?.requestId;
      queueMicrotask(() => listeners.get('chat-finished')?.forEach(callback => callback({ payload: { requestId, status: 'completed', error: null } })));
      return undefined;
    }
    if (command === 'frontend_ready') return false;
    return undefined;
  });
});
afterEach(cleanup);

describe('chat and settings', () => {
  it('starts with Ask anything leftmost and enables capture on Hyprland', async () => {
    render(<App />);
    expect(screen.getByRole('button', { name: 'Ask anything' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('button', { name: 'Ask anything' }).parentElement?.firstElementChild).toBe(screen.getByRole('button', { name: 'Ask anything' }));
    expect(screen.queryByText('Explain screen')).toBeNull();
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'compact' }));
    expect((screen.getByRole('button', { name: 'Use screen' }) as HTMLButtonElement).disabled).toBe(false);
  });
  it('captures, previews and removes screenshots; only explicit Send submits them', async () => {
    const image = { data_url: 'data:image/png;base64,iVBORw0KGgo=', width: 640, height: 480, bytes: 100, label: 'Selected area' };
    const original = mocks.invoke.getMockImplementation()!;
    mocks.invoke.mockImplementation((command: string, ...argumentsValue: unknown[]) => command === 'capture_screen' ? Promise.resolve(image) : original(command, ...argumentsValue));
    render(<App />);
    await waitFor(() => expect((screen.getByRole('button', { name: 'Use screen' }) as HTMLButtonElement).disabled).toBe(false));
    for (const [label, target] of [['Select area', 'region'], ['Entire screen', 'screen'], ['Select window', 'window']]) {
      fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
      fireEvent.click(screen.getByRole('button', { name: label }));
      await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('capture_screen', { target }));
      await waitFor(() => expect(screen.getByRole('button', { name: 'Preview selected area screenshot' })).toBeTruthy());
      fireEvent.click(screen.getByRole('button', { name: 'Preview selected area screenshot' }));
      expect(screen.getByRole('dialog', { name: 'Screenshot preview' })).toBeTruthy();
      fireEvent.click(screen.getByRole('button', { name: 'Close preview' }));
      fireEvent.click(screen.getByRole('button', { name: 'Remove selected area screenshot' }));
      expect(screen.queryByLabelText('Screenshot attachments')).toBeNull();
    }
    fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select area' }));
    await waitFor(() => expect(screen.getByLabelText('Screenshot attachments')).toBeTruthy());
    fireEvent.change(screen.getByLabelText('Your message'), { target: { value: 'What is this?' } });
    expect((screen.getByRole('button', { name: 'Send message to ChatGPT' }) as HTMLButtonElement).disabled).toBe(false);
    expect(mocks.invoke).not.toHaveBeenCalledWith('send_chat', expect.anything());
    fireEvent.click(screen.getByRole('button', { name: 'Send message to ChatGPT' }));
    expect(screen.queryByLabelText('Screenshot attachments')).toBeNull();
    expect(screen.getByText('What is this?')).toBeTruthy();
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('send_chat', expect.objectContaining({
      text: 'What is this?',
      images: [{ dataUrl: image.data_url }],
    })));
    await waitFor(() => expect((screen.getByRole('button', { name: 'Clear chat' }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select area' }));
    await waitFor(() => expect(screen.getByLabelText('Screenshot attachments')).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    fireEvent.click(screen.getByRole('button', { name: 'Back to chat' }));
    expect(screen.getByRole('button', { name: 'View selected area screenshot' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Remove selected area screenshot' }));
    await waitFor(() => expect((screen.getByRole('button', { name: 'Clear chat' }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: 'Clear chat' }));
    await waitFor(() => expect(screen.queryByRole('img')).toBeNull());
    expect(mocks.invoke.mock.calls.every(([command]) => ['get_settings', 'get_status', 'get_codex_connection', 'get_codex_models', 'frontend_ready', 'fit_popup', 'capture_screen', 'send_chat', 'clear_chat'].includes(command))).toBe(true);
  });

  it('preserves the draft after capture cancellation and shows real capture errors', async () => {
    const original = mocks.invoke.getMockImplementation()!;
    let fail = false;
    mocks.invoke.mockImplementation((command: string, ...argumentsValue: unknown[]) => command === 'capture_screen' ? fail ? Promise.reject('grim is unavailable') : Promise.resolve(null) : original(command, ...argumentsValue));
    render(<App />);
    await waitFor(() => expect((screen.getByRole('button', { name: 'Use screen' }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.change(screen.getByLabelText('Your message'), { target: { value: 'Keep my draft' } });
    fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select area' }));
    await waitFor(() => expect(screen.queryByText('Capturing…')).toBeNull());
    expect((screen.getByLabelText('Your message') as HTMLTextAreaElement).value).toBe('Keep my draft');
    expect(screen.queryByLabelText('Screenshot attachments')).toBeNull();
    fail = true;
    fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
    fireEvent.click(screen.getByRole('button', { name: 'Entire screen' }));
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('grim is unavailable'));
  });

  it('closes capture choices on Escape without hiding the popup', async () => {
    render(<App />);
    await waitFor(() => expect((screen.getByRole('button', { name: 'Use screen' }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('group', { name: 'Capture screenshot' })).toBeNull();
    expect(mocks.invoke).not.toHaveBeenCalledWith('hide_popup');
  });

  it('opens capture as a compact popover without expanding the empty window', async () => {
    render(<App />);
    await waitFor(() => expect((screen.getByRole('button', { name: 'Use screen' }) as HTMLButtonElement).disabled).toBe(false));
    screen.getByRole('button', { name: 'Use screen' }).focus();
    fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
    const choices = screen.getByRole('group', { name: 'Capture screenshot' });
    expect(screen.getByRole('button', { name: 'Use screen' }).parentElement?.contains(choices)).toBe(true);
    expect(mocks.invoke).not.toHaveBeenCalledWith('fit_popup', { layout: 'chat' });
    fireEvent.pointerDown(screen.getByLabelText('Your message'));
    expect(screen.queryByRole('group', { name: 'Capture screenshot' })).toBeNull();
  });

  it('supports keyboard navigation within capture choices', async () => {
    render(<App />);
    await waitFor(() => expect((screen.getByRole('button', { name: 'Use screen' }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
    screen.getByRole('button', { name: 'Select area' }).focus();
    fireEvent.keyDown(document.activeElement!, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Entire screen' }));
    fireEvent.keyDown(document.activeElement!, { key: 'End' });
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Select window' }));
  });

  it('limits screenshot attachments to three and keeps sending explicit', async () => {
    const image = { data_url: 'data:image/png;base64,iVBORw0KGgo=', width: 640, height: 480, bytes: 8 * 1024 * 1024, label: 'Window' };
    const original = mocks.invoke.getMockImplementation()!;
    mocks.invoke.mockImplementation((command: string, ...argumentsValue: unknown[]) => command === 'capture_screen' ? Promise.resolve(image) : original(command, ...argumentsValue));
    render(<App />);
    await waitFor(() => expect((screen.getByRole('button', { name: 'Use screen' }) as HTMLButtonElement).disabled).toBe(false));
    for (let index = 0; index < 3; index++) {
      fireEvent.click(screen.getByRole('button', { name: 'Use screen' }));
      fireEvent.click(screen.getByRole('button', { name: 'Select window' }));
      await waitFor(() => expect(screen.getAllByRole('button', { name: 'Preview window screenshot' })).toHaveLength(index + 1));
    }
    expect((screen.getByRole('button', { name: 'Send message to ChatGPT' }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getAllByRole('button', { name: 'Preview window screenshot' })).toHaveLength(3);
    expect(mocks.invoke).not.toHaveBeenCalledWith('send_chat', expect.anything());
  });
  it('refreshes mouse diagnostics when the tracking worker changes status', async () => {
    render(<App />);
    await waitFor(() => expect(mocks.listen).toHaveBeenCalledWith('trigger-status-changed', expect.any(Function)));
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.getByText('Unavailable')).toBeTruthy();
    const original = mocks.invoke.getMockImplementation()!;
    mocks.invoke.mockImplementation((command: string, ...argumentsValue: unknown[]) => command === 'get_status' ? Promise.resolve({ ...capability, mouse_supported: true }) : original(command, ...argumentsValue));
    const callback = mocks.listen.mock.calls.find(([event]) => event === 'trigger-status-changed')![1];
    await act(async () => { callback(); });
    expect(screen.queryByText('Unavailable')).toBeNull();
  });
  it('puts submitted bubbles above the composer and expands the native window', async () => {
    render(<App />);
    await screen.findByText('ChatGPT connected');
    const input = screen.getByLabelText('Your message');
    fireEvent.change(input, { target: { value: 'Hello Wiggy' } });
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: false });
    const conversation = screen.getByRole('log');
    expect(conversation.textContent).toContain('Hello Wiggy');
    expect(Boolean(conversation.compareDocumentPosition(input.closest('form')!) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(true);
    expect((input as HTMLTextAreaElement).value).toBe('');
    expect(screen.getByText('Use screen')).toBeTruthy();
    expect(screen.queryByText('Run command')).toBeNull();
    expect(screen.getByText('Ask anything')).toBeTruthy();
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'chat' }));
    await waitFor(() => expect((screen.getByRole('button', { name: 'Clear chat' }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole('button', { name: 'Clear chat' }));
    await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith('fit_popup', { layout: 'compact' }));
  });

  it('renders streamed assistant text and stops the active reply', async () => {
    const original = mocks.invoke.getMockImplementation()!;
    let activeRequestId = '';
    mocks.invoke.mockImplementation(async (command: string, argumentsValue?: { requestId?: string }) => {
      if (command === 'send_chat') { activeRequestId = argumentsValue?.requestId ?? ''; return undefined; }
      if (command === 'stop_chat') {
        queueMicrotask(() => listeners.get('chat-finished')?.forEach(callback => callback({ payload: { requestId: activeRequestId, status: 'interrupted', error: null } })));
        return undefined;
      }
      return original(command, argumentsValue);
    });
    render(<App />);
    await screen.findByText('ChatGPT connected');
    const input = screen.getByLabelText('Your message');
    fireEvent.change(input, { target: { value: 'Please answer in one sentence.' } });
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: false });
    await waitFor(() => expect(activeRequestId).not.toBe(''));
    act(() => listeners.get('chat-delta')?.forEach(callback => callback({ payload: { requestId: activeRequestId, delta: 'This is a streamed reply.' } })));
    expect(screen.getByText('This is a streamed reply.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Stop' }));
    await waitFor(() => expect((screen.getByRole('button', { name: 'Clear chat' }) as HTMLButtonElement).disabled).toBe(false));
    expect(mocks.invoke).toHaveBeenCalledWith('stop_chat');
    expect(screen.getByRole('article', { name: 'Wiggy reply' }).querySelector('.message')?.getAttribute('data-status')).toBe('interrupted');
  });

  it('keeps Shift+Enter and IME composition from submitting', () => {
    render(<App />);
    const input = screen.getByLabelText('Your message');
    fireEvent.change(input, { target: { value: 'Draft' } });
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    fireEvent.keyDown(input, { key: 'Enter', isComposing: true });
    expect(screen.queryByRole('article')).toBeNull();
  });

  it('offers chat and screenshot capture without a command runner', async () => {
    render(<App />);
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'compact' }));
    expect(screen.getByRole('button', { name: 'Ask anything' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Use screen' })).toBeTruthy();
    expect(screen.queryByText(/Run command|Terminal/)).toBeNull();
    expect(screen.queryByLabelText('Shell command')).toBeNull();
    expect(mocks.invoke.mock.calls.some(([command]) => ['review_command', 'execute_command', 'stop_command'].includes(String(command)))).toBe(false);
  });

  it('uses a trash icon for clearing, a gear for settings and retains hide', async () => {
    render(<App />);
    await screen.findByText('ChatGPT connected');
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'compact' }));
    const input = screen.getByLabelText('Your message');
    fireEvent.change(input, { target: { value: 'Previous message' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    const clear = screen.getByRole('button', { name: 'Clear chat' });
    expect(clear.querySelector('path')?.getAttribute('d')).toBe('M5 7h14M9 7V5h6v2M8 7l1 13h6l1-13');
    const settings = screen.getByRole('button', { name: 'Settings' });
    expect(settings.querySelector('circle')).not.toBeNull();
    expect(settings.querySelector('path')?.getAttribute('d')).toContain('M10 3h4');
    fireEvent.change(input, { target: { value: 'Keep this draft' } });
    await waitFor(() => expect((clear as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(clear);
    await waitFor(() => expect(screen.queryByText('Previous message')).toBeNull());
    expect((input as HTMLTextAreaElement).value).toBe('Keep this draft');
    fireEvent.click(settings);
    expect(screen.getByRole('button', { name: 'Back to chat' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Hide Wiggy' }));
    expect(mocks.invoke).toHaveBeenCalledWith('hide_popup');
  });

  it('keeps chat actions inside the composer and settings free of descriptions', async () => {
    render(<App />);
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'compact' }));
    expect(screen.getAllByRole('heading')).toHaveLength(1);
    expect(screen.getByText('Ask anything').closest('form')).toBe(screen.getByLabelText('Your message').closest('form'));
    expect(screen.queryByText('Ask Agent')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.getAllByRole('heading')).toHaveLength(1);
    expect(screen.getByRole('checkbox', { name: 'Mouse wiggle' })).toBeTruthy();
    expect(screen.queryByText(/Higher sensitivity|Tracking pauses|Choose where|Changes apply/)).toBeNull();
    const advanced = screen.getByText('Advanced').closest('details')!;
    expect(advanced.open).toBe(false);
    expect(advanced.contains(screen.getByText('manual bind'))).toBe(true);
  });

  it('persists compact mouse controls', async () => {
    render(<App />);
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'compact' }));
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Mouse wiggle' }));
    fireEvent.change(screen.getByLabelText('Sensitivity'), { target: { value: '1.5' } });
    fireEvent.change(screen.getByLabelText('Cooldown (milliseconds)'), { target: { value: '1500' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save settings' }));
    await screen.findByText('Settings saved.');
    expect(mocks.invoke).toHaveBeenCalledWith('save_settings', { settings: { ...configuration, mouse_enabled: false, sensitivity: 1.5, cooldown_ms: 1500 } });
  });

  it('expands settings and saves a selected corner while preserving the draft', async () => {
    render(<App />);
    fireEvent.change(screen.getByLabelText('Your message'), { target: { value: 'Keep my draft' } });
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'compact' }));
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'settings' }));
    expect(screen.getAllByRole('button', { name: /Back/ })).toHaveLength(1);
    fireEvent.click(screen.getByLabelText('Bottom right'));
    fireEvent.click(screen.getByRole('button', { name: 'Save settings' }));
    await screen.findByText('Settings saved.');
    expect(mocks.invoke).toHaveBeenCalledWith('save_settings', { settings: { ...configuration, popup_position: 'bottom_right' } });
    fireEvent.click(screen.getByRole('button', { name: 'Back to chat' }));
    expect((screen.getByLabelText('Your message') as HTMLTextAreaElement).value).toBe('Keep my draft');
  });

  it('uses four corner targets and a center cursor target with one selected radio', async () => {
    render(<App />);
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'compact' }));
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.getAllByRole('radio')).toHaveLength(5);
    for (const label of ['Top left', 'Top right', 'Bottom left', 'Bottom right', 'Near cursor']) {
      fireEvent.click(screen.getByRole('radio', { name: label }));
      expect((screen.getByRole('radio', { name: label }) as HTMLInputElement).checked).toBe(true);
      expect(screen.getAllByRole('radio').filter(radio => (radio as HTMLInputElement).checked)).toHaveLength(1);
    }
    fireEvent.click(screen.getByRole('button', { name: 'Save settings' }));
    await screen.findByText('Settings saved.');
    expect(mocks.invoke).toHaveBeenCalledWith('save_settings', { settings: configuration });
  });

  it('loads settings even if compositor diagnostics fail', async () => {
    const original = mocks.invoke.getMockImplementation()!;
    mocks.invoke.mockImplementation((command: string, ...argumentsValue: unknown[]) => command === 'get_status' ? Promise.reject(new Error('IPC unavailable')) : original(command, ...argumentsValue));
    render(<App />);
    await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith('fit_popup', { layout: 'compact' }));
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(screen.getByLabelText('Cooldown (milliseconds)')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Save settings' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('offers retry when settings loading fails', async () => {
    const original = mocks.invoke.getMockImplementation()!;
    mocks.invoke.mockImplementation((command: string, ...argumentsValue: unknown[]) => command === 'get_settings' ? Promise.reject(new Error('Read failed')) : original(command, ...argumentsValue));
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    const retry = await screen.findByRole('button', { name: 'Retry loading' });
    mocks.invoke.mockImplementation(original);
    fireEvent.click(retry);
    await screen.findByLabelText('Top left');
  });
});
