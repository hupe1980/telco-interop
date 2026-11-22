// The playground: telco_interop::report as WebAssembly. Everything runs here; the only request
// is the module itself. Results are inserted as text (textContent), never as markup.
const $ = (id) => document.getElementById(id);
const status = $('pg-status');
const output = $('pg-output');
const input = $('pg-input');
const encoder = new TextEncoder();
const decoder = new TextDecoder();
let raw = null; // the bytes of a dropped or opened file, until the text is edited

async function load() {
  const url = new URL('playground.wasm', import.meta.url);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`module: HTTP ${response.status}`);
  try {
    return (await WebAssembly.instantiateStreaming(response.clone(), {})).instance.exports;
  } catch {
    // A server that does not send application/wasm: instantiate from the bytes instead.
    return (await WebAssembly.instantiate(await response.arrayBuffer(), {})).instance.exports;
  }
}

// Views of memory are made after each call: a call may grow memory and detach older views.
function text(wasm) {
  const ptr = wasm.pg_output_ptr();
  const len = wasm.pg_output_len();
  return decoder.decode(new Uint8Array(wasm.memory.buffer, ptr, len));
}

/** Copies `bytes` (at most the module's limit) into the module's input buffer; returns the length. */
function put(wasm, bytes) {
  const n = Math.min(bytes.length, wasm.pg_input_limit());
  const ptr = wasm.pg_input(n);
  new Uint8Array(wasm.memory.buffer, ptr, n).set(bytes.subarray(0, n));
  return n;
}

function show(wasm, run, what) {
  try {
    const failed = run();
    output.textContent = text(wasm);
    status.textContent = `${what}: ${failed ? 'fails (the command line exits with 1)' : 'passes (exit code 0)'}`;
  } catch {
    output.textContent = '';
    status.textContent = `${what}: the module stopped on this input; please report it.`;
  }
}

async function useFile(file) {
  raw = new Uint8Array(await file.arrayBuffer());
  input.value = decoder.decode(raw);
}

async function main() {
  if (typeof WebAssembly !== 'object') {
    status.textContent = 'This browser does not run WebAssembly; use the command line instead.';
    return;
  }
  let wasm;
  try {
    wasm = await load();
  } catch {
    status.textContent = 'The module could not be loaded; use the command line instead.';
    return;
  }
  const examples = $('pg-example');
  for (let i = 0; wasm.pg_example(i, 0) === 1; i++) {
    examples.add(new Option(text(wasm), String(i)));
  }
  examples.addEventListener('change', () => {
    if (examples.value !== '' && wasm.pg_example(Number(examples.value), 1) === 1) {
      input.value = text(wasm);
      raw = null;
    }
  });
  input.addEventListener('input', () => { raw = null; });
  input.addEventListener('dragover', (e) => e.preventDefault());
  input.addEventListener('drop', (e) => {
    e.preventDefault();
    if (e.dataTransfer.files.length > 0) useFile(e.dataTransfer.files[0]);
  });
  $('pg-file').addEventListener('change', (e) => {
    if (e.target.files.length > 0) useFile(e.target.files[0]);
  });
  const payload = () => put(wasm, raw ?? encoder.encode(input.value));
  $('pg-read').addEventListener('click', () => show(wasm, () => wasm.pg_read(payload()), 'Read'));
  $('pg-check').addEventListener('click', () => show(wasm, () => wasm.pg_check(payload()), 'Check'));
  const code = $('pg-code');
  const lookup = () => show(wasm, () => wasm.pg_code(put(wasm, encoder.encode(code.value.trim())), Number($('pg-scope').value)), 'Code lookup');
  $('pg-lookup').addEventListener('click', lookup);
  code.addEventListener('keydown', (e) => { if (e.key === 'Enter') lookup(); });
  for (const el of document.querySelectorAll('.playground [disabled]')) el.disabled = false;
  status.textContent = 'Ready. Nothing you enter leaves this page.';
}

main();
