import * as vscode from 'vscode';
import * as api from '../api/client';
import type { AppSettingsData } from '../api/client';
import { getApiBaseUrl } from '../config';

const SECTION = 'authorkit';

export async function openLlmSettingsPanel(context: vscode.ExtensionContext): Promise<void> {
  const baseUrl = getApiBaseUrl();
  let initial: AppSettingsData;
  try {
    initial = (await api.getAppSettings(baseUrl)) as AppSettingsData;
    const cfg = vscode.workspace.getConfiguration(SECTION);
    await cfg.update(
      'activeLlmProfile',
      initial.active_llm_config || '',
      vscode.ConfigurationTarget.Global
    );
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(
      vscode.l10n.t(
        'Could not load LLM settings from {0}. Is the API running? {1}',
        baseUrl,
        msg
      )
    );
    return;
  }

  const panel = vscode.window.createWebviewPanel(
    'authorkitLlmSettings',
    vscode.l10n.t('LLM settings'),
    vscode.ViewColumn.One,
    { enableScripts: true, retainContextWhenHidden: true }
  );

  panel.webview.html = getWebviewHtml(initial);

  panel.webview.onDidReceiveMessage(
    async (msg: { type?: string; settings?: AppSettingsData }) => {
      if (msg.type !== 'save' || !msg.settings) {
        return;
      }
      try {
        await api.putAppSettings(baseUrl, msg.settings);
        const cfg = vscode.workspace.getConfiguration(SECTION);
        await cfg.update(
          'activeLlmProfile',
          msg.settings.active_llm_config || '',
          vscode.ConfigurationTarget.Global
        );
        void vscode.window.showInformationMessage(
          vscode.l10n.t('LLM settings saved (API + editor).')
        );
      } catch (e) {
        const m = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(vscode.l10n.t('Save failed: {0}', m));
      }
    },
    undefined,
    context.subscriptions
  );
}

function getWebviewHtml(initial: AppSettingsData): string {
  const boot = JSON.stringify(initial).replace(/</g, '\\u003c');
  const ui = {
    hint: vscode.l10n.t(
      'Saved to the API settings file (same as <code>~/.authorkit/settings.json</code> by default). Profile name must match a key under <code>llm_configs</code>; it is sent as <code>provider</code> for workshop chat.'
    ),
    h1: vscode.l10n.t('LLM profiles'),
    activeProfile: vscode.l10n.t(
      'Active profile <span class="ro">(used by the API as default)</span>'
    ),
    endpoint: vscode.l10n.t('Endpoint'),
    model: vscode.l10n.t('Model'),
    apiKey: vscode.l10n.t('API key'),
    timeout: vscode.l10n.t('Timeout (seconds)'),
    save: vscode.l10n.t('Save to API'),
    provPrefix: vscode.l10n.t('Provider class: '),
  };
  const uiJson = JSON.stringify(ui).replace(/</g, '\\u003c');
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <style>
    body {
      font-family: var(--vscode-font-family);
      color: var(--vscode-foreground);
      padding: 1rem;
      max-width: 640px;
    }
    h1 { font-size: 1.1rem; margin: 0 0 0.5rem 0; }
    p.hint { opacity: 0.85; font-size: 0.85rem; margin: 0 0 1rem 0; }
    label { display: block; margin-top: 0.65rem; font-size: 0.85rem; }
    input, select {
      width: 100%; box-sizing: border-box; margin-top: 0.2rem;
      background: var(--vscode-input-background);
      color: var(--vscode-input-foreground);
      border: 1px solid var(--vscode-input-border, #555);
      padding: 0.35rem 0.5rem;
    }
    button {
      margin-top: 1rem; padding: 0.45rem 1rem;
      background: var(--vscode-button-background);
      color: var(--vscode-button-foreground);
      border: none; cursor: pointer;
    }
    button:hover { background: var(--vscode-button-hoverBackground); }
    .ro { opacity: 0.9; font-size: 0.8rem; }
  </style>
</head>
<body>
  <h1 id="h1"></h1>
  <p class="hint" id="hint"></p>
  <label><span id="activeLbl"></span>
    <select id="active"></select>
  </label>
  <p class="ro" id="provLabel"></p>
  <label><span id="endpointLbl"></span>
    <input type="text" id="endpoint" autocomplete="off" />
  </label>
  <label><span id="modelLbl"></span>
    <input type="text" id="model" autocomplete="off" />
  </label>
  <label><span id="apiKeyLbl"></span>
    <input type="password" id="api_key" autocomplete="off" />
  </label>
  <label><span id="timeoutLbl"></span>
    <input type="number" id="timeout" min="1" step="1" />
  </label>
  <div>
    <button type="button" id="save"></button>
  </div>
  <script>
    (function() {
      const vscode = acquireVsCodeApi();
      var ui = ${uiJson};
      document.getElementById('h1').textContent = ui.h1;
      document.getElementById('hint').innerHTML = ui.hint;
      document.getElementById('activeLbl').innerHTML = ui.activeProfile;
      document.getElementById('endpointLbl').textContent = ui.endpoint;
      document.getElementById('modelLbl').textContent = ui.model;
      document.getElementById('apiKeyLbl').textContent = ui.apiKey;
      document.getElementById('timeoutLbl').textContent = ui.timeout;
      document.getElementById('save').textContent = ui.save;
      var app = ${boot};
      var editingKey = app.active_llm_config || Object.keys(app.llm_configs || {})[0] || '';

      function fillActiveOptions() {
        var sel = document.getElementById('active');
        sel.innerHTML = '';
        var keys = Object.keys(app.llm_configs || {});
        keys.forEach(function(k) {
          var opt = document.createElement('option');
          opt.value = k; opt.textContent = k;
          sel.appendChild(opt);
        });
        if (keys.indexOf(app.active_llm_config) >= 0) {
          sel.value = app.active_llm_config;
        } else if (keys.length) {
          sel.value = keys[0];
        }
        editingKey = sel.value;
      }

      function persistFields() {
        if (!editingKey || !app.llm_configs[editingKey]) return;
        var c = app.llm_configs[editingKey];
        c.endpoint = document.getElementById('endpoint').value.trim();
        c.model = document.getElementById('model').value.trim();
        c.api_key = document.getElementById('api_key').value;
        var t = parseInt(document.getElementById('timeout').value, 10);
        c.timeout = isNaN(t) ? 30 : t;
      }

      function showFields() {
        var c = app.llm_configs[editingKey];
        if (!c) return;
        document.getElementById('provLabel').textContent = ui.provPrefix + (c.provider || '');
        document.getElementById('endpoint').value = c.endpoint || '';
        document.getElementById('model').value = c.model || '';
        document.getElementById('api_key').value = c.api_key || '';
        document.getElementById('timeout').value = String(c.timeout != null ? c.timeout : 30);
      }

      document.getElementById('active').addEventListener('change', function() {
        persistFields();
        editingKey = document.getElementById('active').value;
        app.active_llm_config = editingKey;
        showFields();
      });

      document.getElementById('save').addEventListener('click', function() {
        persistFields();
        vscode.postMessage({ type: 'save', settings: app });
      });

      fillActiveOptions();
      showFields();
    })();
  </script>
</body>
</html>`;
}
