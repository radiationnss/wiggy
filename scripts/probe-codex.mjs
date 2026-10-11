import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { createConnection } from 'node:net';
import { PassThrough } from 'node:stream';
import WebSocket from 'ws';
import { createServer } from 'node:http';
import { gunzipSync, zstdDecompressSync } from 'node:zlib';

const daemon = JSON.parse(execFileSync('codex', ['app-server', 'daemon', 'version'], { encoding: 'utf8' }));
if (daemon.status !== 'running' || !daemon.socketPath) throw new Error('Shared Codex daemon is not running');
function websocketClient() {
  const connection = new WebSocket('ws://localhost/', { createConnection: () => createConnection(daemon.socketPath), maxPayload: 4 * 1024 * 1024 });
  const stdout = new PassThrough();
  connection.on('message', bytes => { stdout.write(bytes); stdout.write('\n'); });
  connection.on('close', () => stdout.end());
  return {
    stdin: {
      write: text => {
        for (const line of text.trim().split('\n')) {
          if (connection.readyState === WebSocket.OPEN) connection.send(line);
          else connection.once('open', () => connection.send(line));
        }
      },
      end: () => connection.close(),
    },
    stdout,
    stderr: new PassThrough(),
    on: (event, callback) => connection.on(event === 'exit' ? 'close' : event, callback),
    kill: () => connection.terminate(),
  };
}
const direct = process.argv.includes('--direct-socket');
const socket = direct ? createConnection(daemon.socketPath) : null;
const child = direct ? {
  stdin: socket,
  stdout: socket,
  stderr: new PassThrough(),
  on: (event, callback) => socket.on(event === 'exit' ? 'close' : event, callback),
  kill: () => socket.destroy(),
} : process.argv.includes('--websocket') ? websocketClient() : spawn('codex', ['app-server', 'proxy', '--sock', daemon.socketPath], { stdio: ['pipe', 'pipe', 'pipe'] });
const pending = new Map();
let nextId = 0;
let stderrBytes = 0;
let stdoutBytes = 0;
let notificationCount = 0;
let finishedTurnStatus;
const observedItemTypes = new Set();
let assistantDeltaText = '';
let assistantDeltaContext;
let liveInferenceRequested = false;
child.stderr.on('data', bytes => { stderrBytes += bytes.length; });
child.stdout.on('data', bytes => { stdoutBytes += bytes.length; });
const lines = createInterface({ input: child.stdout });
lines.on('line', line => {
  if (line.length > 4 * 1024 * 1024) { child.kill(); return; }
  let message;
  try { message = JSON.parse(line); } catch { child.kill(); return; }
  if (message.id !== undefined && pending.has(message.id)) {
    const entry = pending.get(message.id);
    pending.delete(message.id);
    clearTimeout(entry.timer);
    if (message.error) entry.reject(new Error(`RPC error ${message.error.code}: ${message.error.message}`));
    else entry.resolve(message.result);
  } else if (message.method && message.id === undefined) {
    notificationCount++;
    if (message.method === 'turn/completed') finishedTurnStatus = message.params?.turn?.status;
    if (message.params?.item?.type) observedItemTypes.add(message.params.item.type);
    if (message.method === 'item/agentMessage/delta' && typeof message.params?.delta === 'string') {
      assistantDeltaText += message.params.delta;
      assistantDeltaContext = { threadId: message.params.threadId, turnId: message.params.turnId };
    }
  } else if (message.id !== undefined) {
    child.stdin.write(`${JSON.stringify({ id: message.id, error: { code: -32601, message: 'Probe does not permit actions' } })}\n`);
  }
});
child.on('error', fail);
child.on('exit', () => fail(new Error('Proxy exited')));
function fail(error) {
  for (const entry of pending.values()) { clearTimeout(entry.timer); entry.reject(error); }
  pending.clear();
}
function request(method, params) {
  const id = ++nextId;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`${method} timed out`)); }, 15000);
    pending.set(id, { resolve, reject, timer });
    child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
  });
}

let directory;
let fixture;
let capturedToolNames;
let authorizationPresent = false;
let fixtureRequestCount = 0;
let canaryRead = false;
const canaryText = 'WIGGY_LOCAL_FIXTURE_FILE_MUST_NOT_BE_READ';
try {
  await request('initialize', { clientInfo: { name: 'wiggy_probe', title: 'Wiggy feasibility probe', version: '0.1.0' }, capabilities: { experimentalApi: true } });
  child.stdin.write(`${JSON.stringify({ method: 'initialized' })}\n`);
  const account = await request('account/read', { refreshToken: false });
  console.log(JSON.stringify({ daemonVersion: daemon.appServerVersion, authType: account.account?.type ?? null, requiresOpenaiAuth: account.requiresOpenaiAuth }));
  const models = await request('model/list', { limit: 20, includeHidden: false });
  console.log(JSON.stringify({ models: models.data.map(model => ({ id: model.id, modalities: model.inputModalities, isDefault: model.isDefault })) }));
  const auditTools = process.argv.includes('--audit-tools');
  const adversarial = process.argv.includes('--adversarial');
  const liveChatSmoke = process.argv.includes('--live-chat-smoke');
  let baseUrl;
  if (auditTools) {
    fixture = createServer(async (incoming, outgoing) => {
      authorizationPresent ||= Boolean(incoming.headers.authorization);
      const chunks = [];
      let count = 0;
      for await (const bytes of incoming) {
        count += bytes.length;
        if (count > 4 * 1024 * 1024) { outgoing.writeHead(413).end(); return; }
        chunks.push(bytes);
      }
      try {
        let bytes = Buffer.concat(chunks);
        if (incoming.headers['content-encoding'] === 'gzip') bytes = gunzipSync(bytes);
        if (incoming.headers['content-encoding'] === 'zstd') bytes = zstdDecompressSync(bytes);
        const payload = JSON.parse(bytes.toString('utf8'));
        fixtureRequestCount++;
        canaryRead ||= (payload.input ?? []).some(item => item.type === 'function_call_output' && JSON.stringify(item.output).includes(canaryText));
        capturedToolNames = (payload.tools ?? []).map(tool => tool.name ?? tool.type);
        outgoing.writeHead(200, { 'Content-Type': 'text/event-stream' });
        const output = adversarial && fixtureRequestCount === 1 ? [{
          type: 'function_call', id: 'wiggy-fake-call', call_id: 'wiggy-fake-call',
          name: 'exec_command', arguments: JSON.stringify({ cmd: `cat '${join(directory, 'canary.txt')}'`, yield_time_ms: 1000, max_output_tokens: 100 }), status: 'completed',
        }] : [{
          type: 'message', id: `wiggy-fake-message-${fixtureRequestCount}`, role: 'assistant', status: 'completed',
          content: [{ type: 'output_text', text: 'Wiggy fixture reply.' }],
        }];
        if (output[0].type === 'function_call') {
          outgoing.write(`event: response.output_item.added\ndata: ${JSON.stringify({ type: 'response.output_item.added', output_index: 0, item: { ...output[0], arguments: '' } })}\n\n`);
          outgoing.write(`event: response.function_call_arguments.delta\ndata: ${JSON.stringify({ type: 'response.function_call_arguments.delta', item_id: output[0].id, output_index: 0, delta: output[0].arguments })}\n\n`);
          outgoing.write(`event: response.output_item.done\ndata: ${JSON.stringify({ type: 'response.output_item.done', output_index: 0, item: output[0] })}\n\n`);
        } else {
          outgoing.write(`event: response.output_item.added\ndata: ${JSON.stringify({ type: 'response.output_item.added', output_index: 0, item: { ...output[0], content: [] } })}\n\n`);
          outgoing.write(`event: response.output_text.delta\ndata: ${JSON.stringify({ type: 'response.output_text.delta', item_id: output[0].id, output_index: 0, content_index: 0, delta: 'Wiggy fixture reply.' })}\n\n`);
          outgoing.write(`event: response.output_item.done\ndata: ${JSON.stringify({ type: 'response.output_item.done', output_index: 0, item: output[0] })}\n\n`);
        }
        for (const [type, status] of [['response.created', 'in_progress'], ['response.completed', 'completed']]) {
          outgoing.write(`event: ${type}\ndata: ${JSON.stringify({ type, response: { id: `wiggy-local-fixture-${fixtureRequestCount}`, object: 'response', status, output: type === 'response.completed' ? output : [], usage: { input_tokens: 0, output_tokens: 0, total_tokens: 0 } } })}\n\n`);
        }
        outgoing.end();
      } catch {
        outgoing.writeHead(400).end();
      }
    });
    await new Promise(resolve => fixture.listen(0, '127.0.0.1', resolve));
    baseUrl = `http://127.0.0.1:${fixture.address().port}/v1`;
  }
  if (process.argv.includes('--thread') || auditTools || liveChatSmoke) {
    directory = await mkdtemp(join(tmpdir(), 'wiggy-probe-'));
    if (adversarial) await writeFile(join(directory, 'canary.txt'), canaryText, { mode: 0o600 });
    const result = await request('thread/start', {
      cwd: directory,
      ephemeral: true,
      environments: [],
      runtimeWorkspaceRoots: [],
      dynamicTools: [],
      approvalPolicy: 'untrusted',
      sandbox: 'read-only',
      ...(auditTools ? { modelProvider: 'wiggy_local_fixture', model: 'gpt-6-luna', allowProviderModelFallback: false } : {}),
      baseInstructions: 'You are Wiggy, a chat assistant. Use only messages and images explicitly supplied in this conversation.',
      config: {
        project_doc_max_bytes: 0,
        web_search: 'disabled',
        mcp_servers: {},
        features: { shell_tool: false, unified_exec: false, code_mode_host: false, code_mode: false, hooks: false, apps: false, plugins: false, multi_agent: false, memories: false, goals: false, view_image: false, image_generation: false, skip_host_skill_discovery: true, enable_request_compression: false },
        ...(auditTools ? { model_providers: { wiggy_local_fixture: { name: 'Wiggy local safety fixture', base_url: baseUrl, wire_api: 'responses', requires_openai_auth: false, supports_websockets: false } } } : {}),
      },
    });
    console.log(JSON.stringify({ threadCreated: Boolean(result.thread?.id), ephemeral: result.thread?.ephemeral, instructionSourceCount: result.instructionSources?.length, workspaceRootCount: result.runtimeWorkspaceRoots?.length, model: result.model, modelProvider: result.modelProvider }));
    if (!result.thread?.id || result.thread.ephemeral !== true || result.instructionSources?.length !== 0 || result.runtimeWorkspaceRoots?.length !== 0) {
      throw new Error('Isolated ephemeral thread requirements were not satisfied');
    }
    if (auditTools && result.modelProvider !== 'wiggy_local_fixture') throw new Error('Local fixture provider was not selected; refusing to start a turn');
    if (result.thread?.id) {
      const features = await request('experimentalFeature/list', { threadId: result.thread.id, limit: 200 });
      const checked = ['shell_tool', 'unified_exec', 'code_mode_host', 'hooks', 'apps', 'plugins', 'multi_agent', 'memories', 'goals', 'view_image', 'image_generation'];
      console.log(JSON.stringify({ threadFeatures: features.data.filter(feature => checked.includes(feature.name)).map(feature => ({ name: feature.name, enabled: feature.enabled })) }));
      const apps = await request('app/installed', { threadId: result.thread.id, forceRefresh: false });
      const callableAppCount = apps.apps.filter(app => app.callable).length;
      console.log(JSON.stringify({ callableAppCount }));
      if (callableAppCount !== 0) throw new Error('A connected app is callable in the chat-only thread');
      const servers = await request('mcpServerStatus/list', { threadId: result.thread.id, detail: 'toolsAndAuthOnly', limit: 100 });
      console.log(JSON.stringify({ mcpServerCount: servers.data?.length, mcpToolsCount: servers.data?.reduce((count, server) => count + Object.keys(server.tools ?? {}).length, 0) }));
      if (auditTools) {
        const started = await request('turn/start', { threadId: result.thread.id, environments: [], input: [{ type: 'text', text: 'Local safety fixture. Do not use tools.' }] });
        const expectedTurnId = started.turn?.id;
        const deadline = Date.now() + 10000;
        while (finishedTurnStatus === undefined && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 50));
        console.log(JSON.stringify({ localFixtureToolNames: capturedToolNames ?? null, authorizationPresent, realProviderInferenceRequested: false, fixtureRequestCount, finishedTurnStatus, observedItemTypes: [...observedItemTypes], assistantDeltaText, assistantDeltaContext, canaryRead }));
        const allowedItems = new Set(['userMessage', 'agentMessage', 'reasoning']);
        if (capturedToolNames === undefined) throw new Error('Local fixture did not receive the model request');
        if (authorizationPresent || canaryRead || capturedToolNames.length) throw new Error('Safety audit failed: request exposed tools or fixture canary was read');
        if (finishedTurnStatus !== 'completed') throw new Error('Local adversarial turn did not reach a terminal completed status');
        if ([...observedItemTypes].some(type => !allowedItems.has(type))) throw new Error('Safety audit observed a non-chat item');
        if (!assistantDeltaText.includes('Wiggy fixture reply.')) throw new Error('Local fixture did not produce an assistant message delta');
        if (assistantDeltaContext?.threadId !== result.thread.id || assistantDeltaContext?.turnId !== expectedTurnId) throw new Error('Assistant delta was not scoped to the expected thread and turn');
        // Ephemeral threads only survive while this socket remains subscribed.
        // Exercise the next user turn on the same connection, as Wiggy does.
        finishedTurnStatus = undefined;
        assistantDeltaText = '';
        assistantDeltaContext = undefined;
        observedItemTypes.clear();
        const followup = await request('turn/start', { threadId: result.thread.id, environments: [], input: [{ type: 'text', text: 'Reply with the test sentence once more.' }] });
        const followupTurnId = followup.turn?.id;
        const followupDeadline = Date.now() + 10000;
        while (finishedTurnStatus === undefined && Date.now() < followupDeadline) await new Promise(resolve => setTimeout(resolve, 50));
        console.log(JSON.stringify({ persistentEphemeralSession: true, followupTurnStatus: finishedTurnStatus, followupDeltaText: assistantDeltaText, authorizationPresent, canaryRead }));
        if (finishedTurnStatus !== 'completed' || !assistantDeltaText.includes('Wiggy fixture reply.') || assistantDeltaContext?.threadId !== result.thread.id || assistantDeltaContext?.turnId !== followupTurnId || authorizationPresent || canaryRead || [...observedItemTypes].some(type => !allowedItems.has(type))) {
          throw new Error('Ephemeral follow-up safety or streaming check failed');
        }
      }
      if (liveChatSmoke) {
        if (account.account?.type !== 'chatgpt') throw new Error('Live smoke test requires the existing Codex ChatGPT login');
        finishedTurnStatus = undefined;
        assistantDeltaText = '';
        assistantDeltaContext = undefined;
        const started = await request('turn/start', {
          threadId: result.thread.id,
          environments: [],
          input: [{ type: 'text', text: 'Please reply briefly to confirm this live Wiggy chat connection works.' }],
        });
        liveInferenceRequested = true;
        const turnId = started.turn?.id;
        if (!turnId) throw new Error('Live smoke test did not start a turn');
        const deadline = Date.now() + 120000;
        while (finishedTurnStatus === undefined && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 50));
        console.log(JSON.stringify({ liveChatSmoke: true, turnStatus: finishedTurnStatus, assistantText: assistantDeltaText, threadId: result.thread.id, turnId, authType: account.account.type }));
        if (finishedTurnStatus !== 'completed' || !assistantDeltaText.trim() || assistantDeltaContext?.threadId !== result.thread.id || assistantDeltaContext?.turnId !== turnId) {
          throw new Error('Live ChatGPT smoke test failed to produce a completed assistant reply');
        }
      }
      await request('thread/unsubscribe', { threadId: result.thread.id });
    }
  }
  console.log(JSON.stringify({ notificationCount, stderrBytes, inferenceRequested: liveInferenceRequested }));
} catch (error) {
  console.log(JSON.stringify({ failed: error.message, stdoutBytes, stderrBytes, notificationCount, inferenceRequested: liveInferenceRequested }));
  process.exitCode = 1;
} finally {
  lines.close();
  child.stdin.end();
  child.kill('SIGTERM');
  if (fixture) { fixture.closeAllConnections(); await new Promise(resolve => fixture.close(resolve)); }
  if (directory) await rm(directory, { recursive: true });
}
