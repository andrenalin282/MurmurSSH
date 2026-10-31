import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api/index";
import type { AiCaps, AiStatus, AiTokenInput, AiTokenView, Profile } from "../types";
import { t } from "../i18n/index";
import { showConfirm } from "./dialog";
import { installModalKeyHandler } from "./modal-keys";

const CAP_KEYS: Array<[keyof AiCaps, string]> = [
  ["read", "ai.capRead"],
  ["download", "ai.capDownload"],
  ["upload", "ai.capUpload"],
  ["write", "ai.capWrite"],
  ["delete", "ai.capDelete"],
  ["exec", "ai.capExec"],
  ["confirm_writes", "ai.capConfirmWrites"],
];

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (className) e.className = className;
  if (text !== undefined) e.textContent = text;
  return e;
}

function fmtDate(unix: number): string {
  return new Date(unix * 1000).toLocaleString();
}

function button(label: string, secondary = true): HTMLButtonElement {
  const b = el("button", secondary ? "btn-secondary" : "", label);
  b.type = "button";
  return b;
}

async function copyText(text: string, btn: HTMLButtonElement): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    const old = btn.textContent;
    btn.textContent = t("ai.copied");
    setTimeout(() => {
      btn.textContent = old;
    }, 1500);
  } catch {
    /* clipboard unavailable: user can still select the readonly field */
  }
}

export async function showAiAccessDialog(): Promise<void> {
  installModalKeyHandler();

  const overlay = el("div", "modal-overlay");
  const modal = el("div", "modal modal--form");
  modal.setAttribute("role", "dialog");
  modal.setAttribute("aria-modal", "true");
  overlay.appendChild(modal);
  document.body.appendChild(overlay);

  const close = () => overlay.remove();

  const renderError = (err: unknown) => {
    modal.replaceChildren();
    modal.appendChild(el("div", "modal__title", t("ai.title")));
    modal.appendChild(el("div", "form-error", t("ai.loadError", { error: String(err) })));
    const actions = el("div", "modal__actions");
    const c = button(t("ai.close"));
    c.setAttribute("data-modal-cancel", "");
    c.addEventListener("click", close);
    actions.appendChild(c);
    modal.appendChild(actions);
  };

  // ── List view ───────────────────────────────────────────────────────────
  const renderList = async () => {
    let status: AiStatus;
    let tokens: AiTokenView[];
    let profiles: Profile[];
    try {
      [status, tokens, profiles] = await Promise.all([
        api.aiStatus(),
        api.aiListTokens(),
        api.listProfiles(),
      ]);
    } catch (err) {
      renderError(err);
      return;
    }
    const profileNames = new Map(profiles.map((p) => [p.id, p.name]));

    modal.replaceChildren();
    modal.appendChild(el("div", "modal__title", t("ai.title")));

    // Master switch
    const enField = el("div", "form-field");
    const enLabel = el("label");
    const enBox = el("input");
    enBox.type = "checkbox";
    enBox.checked = status.enabled;
    enLabel.append(enBox, document.createTextNode(" " + t("ai.enableLabel")));
    enField.appendChild(enLabel);
    enField.appendChild(el("div", "form-field__hint", t("ai.enableHint")));
    const statusLine = el("div", "form-field__hint");
    const auditLine = el("div", "form-field__hint");
    const setStatus = (s: AiStatus) => {
      statusLine.textContent =
        (s.listening ? t("ai.listeningYes") : t("ai.listeningNo")) +
        " — " +
        t("ai.socket") +
        ": " +
        s.socket_path;
      auditLine.textContent = t("ai.audit") + ": " + s.audit_path;
    };
    setStatus(status);
    enField.append(statusLine, auditLine);
    const enErr = el("div", "form-error");
    enErr.style.display = "none";
    enField.appendChild(enErr);
    enBox.addEventListener("change", async () => {
      enBox.disabled = true;
      enErr.style.display = "none";
      try {
        setStatus(await api.aiSetEnabled(enBox.checked));
      } catch (err) {
        enBox.checked = !enBox.checked;
        enErr.textContent = String(err);
        enErr.style.display = "";
      } finally {
        enBox.disabled = false;
      }
    });
    modal.appendChild(enField);

    // Token list
    const listField = el("div", "form-field");
    listField.appendChild(el("label", "", t("ai.tokens")));
    if (tokens.length === 0) {
      listField.appendChild(el("div", "form-field__hint", t("ai.noTokens")));
    }
    for (const tok of tokens) {
      const row = el("div", "settings-path-display");
      row.style.marginBottom = "8px";
      row.style.whiteSpace = "normal";
      const head = el("div");
      head.style.fontWeight = "600";
      head.textContent = `${tok.name}  (${tok.prefix}…)`;
      row.appendChild(head);
      const profs = tok.profiles.map((id) => profileNames.get(id) ?? id).join(", ");
      row.appendChild(el("div", "form-field__hint", `${t("ai.profilesLabel")}: ${profs || "–"}`));
      const caps = CAP_KEYS.filter(([k]) => tok.caps[k])
        .map(([, key]) => t(key))
        .join(", ");
      row.appendChild(el("div", "form-field__hint", `${t("ai.capsLabel")}: ${caps || "–"}`));
      row.appendChild(
        el(
          "div",
          "form-field__hint",
          (tok.expires_at ? t("ai.expiresOn", { date: fmtDate(tok.expires_at) }) : t("ai.expiresNever")) +
            " · " +
            (tok.last_used_at ? t("ai.lastUsed", { date: fmtDate(tok.last_used_at) }) : t("ai.lastUsedNever"))
        )
      );
      const btns = el("div", "form-field__row");
      btns.style.gap = "8px";
      btns.style.marginTop = "6px";
      const editBtn = button(t("ai.edit"));
      editBtn.addEventListener("click", () => renderForm(profiles, tok));
      const revBtn = button(t("ai.revoke"));
      revBtn.addEventListener("click", async () => {
        if (!(await showConfirm(t("ai.revokeConfirm", { name: tok.name }), t("ai.revoke")))) return;
        try {
          await api.aiRevokeToken(tok.id);
        } catch (err) {
          enErr.textContent = String(err);
          enErr.style.display = "";
          return;
        }
        await renderList();
      });
      btns.append(editBtn, revBtn);
      row.appendChild(btns);
      listField.appendChild(row);
    }
    const newBtn = button(t("ai.newToken"));
    newBtn.addEventListener("click", () => renderForm(profiles, null));
    listField.appendChild(newBtn);
    modal.appendChild(listField);

    const actions = el("div", "modal__actions");
    const closeBtn = button(t("ai.close"));
    closeBtn.setAttribute("data-modal-cancel", "");
    closeBtn.addEventListener("click", close);
    actions.appendChild(closeBtn);
    modal.appendChild(actions);
  };

  // ── Create / edit form ──────────────────────────────────────────────────
  const renderForm = (profiles: Profile[], existing: AiTokenView | null) => {
    modal.replaceChildren();
    modal.appendChild(el("div", "modal__title", existing ? t("ai.formEdit") : t("ai.formNew")));

    const nameField = el("div", "form-field");
    nameField.appendChild(el("label", "", t("ai.name")));
    const nameInput = el("input");
    nameInput.type = "text";
    nameInput.value = existing?.name ?? "";
    nameInput.placeholder = t("ai.namePlaceholder");
    nameField.appendChild(nameInput);
    modal.appendChild(nameField);

    const profField = el("div", "form-field");
    profField.appendChild(el("label", "", t("ai.profilesLabel")));
    const profBoxes: Array<[string, HTMLInputElement]> = [];
    if (profiles.length === 0) {
      profField.appendChild(el("div", "form-field__hint", t("ai.noProfilesAvail")));
    }
    for (const p of profiles) {
      const l = el("label");
      l.style.display = "block";
      const cb = el("input");
      cb.type = "checkbox";
      cb.checked = existing?.profiles.includes(p.id) ?? false;
      l.append(cb, document.createTextNode(" " + p.name));
      profField.appendChild(l);
      profBoxes.push([p.id, cb]);
    }
    modal.appendChild(profField);

    const capField = el("div", "form-field");
    capField.appendChild(el("label", "", t("ai.capsLabel")));
    const defaults: AiCaps = {
      read: true,
      download: true,
      upload: false,
      write: false,
      delete: false,
      exec: false,
      confirm_writes: true,
    };
    const initial = existing?.caps ?? defaults;
    const capBoxes: Array<[keyof AiCaps, HTMLInputElement]> = [];
    for (const [k, key] of CAP_KEYS) {
      const l = el("label");
      l.style.display = "block";
      const cb = el("input");
      cb.type = "checkbox";
      cb.checked = initial[k];
      l.append(cb, document.createTextNode(" " + t(key)));
      capField.appendChild(l);
      capBoxes.push([k, cb]);
    }
    capField.appendChild(el("div", "form-field__hint", t("ai.confirmHint")));
    modal.appendChild(capField);

    const remoteField = el("div", "form-field");
    remoteField.appendChild(el("label", "", t("ai.remoteRoot")));
    const remoteInput = el("input");
    remoteInput.type = "text";
    remoteInput.spellcheck = false;
    remoteInput.value = existing?.remote_root ?? "";
    remoteInput.placeholder = "/var/www";
    remoteField.appendChild(remoteInput);
    remoteField.appendChild(el("div", "form-field__hint", t("ai.remoteRootHint")));
    modal.appendChild(remoteField);

    const localField = el("div", "form-field");
    localField.appendChild(el("label", "", t("ai.localRoot")));
    const localRow = el("div", "form-field__row");
    const localInput = el("input");
    localInput.type = "text";
    localInput.spellcheck = false;
    localInput.value = existing?.local_root ?? "";
    const browse = button(t("common.browse"), false);
    browse.addEventListener("click", async () => {
      const sel = await open({ directory: true, multiple: false });
      if (sel && typeof sel === "string") localInput.value = sel;
    });
    localRow.append(localInput, browse);
    localField.appendChild(localRow);
    localField.appendChild(el("div", "form-field__hint", t("ai.localRootHint")));
    modal.appendChild(localField);

    const expField = el("div", "form-field");
    expField.appendChild(el("label", "", t("ai.expiresDays")));
    const expInput = el("input");
    expInput.type = "number";
    expInput.min = "1";
    expInput.step = "1";
    expInput.style.width = "100px";
    if (existing?.expires_at) {
      const days = Math.ceil((existing.expires_at - Date.now() / 1000) / 86400);
      if (days > 0) expInput.value = String(days);
    }
    expField.appendChild(expInput);
    expField.appendChild(el("div", "form-field__hint", t("ai.expiresDaysHint")));
    modal.appendChild(expField);

    const errEl = el("div", "form-error");
    errEl.style.display = "none";
    modal.appendChild(errEl);
    const showErr = (msg: string) => {
      errEl.textContent = msg;
      errEl.style.display = "";
    };

    const actions = el("div", "modal__actions");
    const cancel = button(t("common.cancel"));
    cancel.setAttribute("data-modal-cancel", "");
    cancel.addEventListener("click", () => void renderList());
    const save = button(existing ? t("ai.save") : t("ai.create"), false);
    save.setAttribute("data-modal-primary", "");
    save.addEventListener("click", async () => {
      const name = nameInput.value.trim();
      if (!name) {
        nameInput.focus();
        return showErr(t("ai.errName"));
      }
      const selected = profBoxes.filter(([, cb]) => cb.checked).map(([id]) => id);
      if (selected.length === 0) return showErr(t("ai.errProfiles"));
      const caps = { ...defaults } as AiCaps;
      for (const [k, cb] of capBoxes) caps[k] = cb.checked;
      const days = parseInt(expInput.value, 10);
      const input: AiTokenInput = {
        name,
        profiles: selected,
        caps,
        remote_root: remoteInput.value.trim() || null,
        local_root: localInput.value.trim() || null,
        expires_in_days: Number.isNaN(days) || days < 1 ? null : days,
      };
      save.disabled = true;
      try {
        if (existing) {
          await api.aiUpdateToken(existing.id, input);
          await renderList();
        } else {
          const res = await api.aiCreateToken(input);
          await renderSecret(res.secret);
        }
      } catch (err) {
        save.disabled = false;
        showErr(t("ai.errSave", { error: String(err) }));
      }
    });
    actions.append(cancel, save);
    modal.appendChild(actions);
    setTimeout(() => nameInput.focus(), 10);
  };

  // ── One-time secret view ────────────────────────────────────────────────
  const renderSecret = async (secret: string) => {
    let bridge = "murmurssh --mcp";
    try {
      bridge = await api.aiBridgeCommand();
    } catch {
      /* fall back to default */
    }
    // User scope: keeps the token out of a project's .mcp.json (which may be committed).
    const command = `claude mcp add -s user murmurssh -e MURMURSSH_TOKEN=${secret} -- ${bridge}`;

    modal.replaceChildren();
    modal.appendChild(el("div", "modal__title", t("ai.secretTitle")));
    modal.appendChild(el("div", "form-error", t("ai.secretWarning")));

    const mk = (label: string, value: string) => {
      const f = el("div", "form-field");
      f.appendChild(el("label", "", label));
      const row = el("div", "form-field__row");
      const inp = el("input");
      inp.type = "text";
      inp.readOnly = true;
      inp.spellcheck = false;
      inp.value = value;
      inp.addEventListener("focus", () => inp.select());
      const cp = button(t("ai.copy"), false);
      cp.addEventListener("click", () => void copyText(value, cp));
      row.append(inp, cp);
      f.appendChild(row);
      modal.appendChild(f);
    };
    mk(t("ai.secretLabel"), secret);
    mk(t("ai.commandLabel"), command);

    const actions = el("div", "modal__actions");
    const done = button(t("ai.close"), false);
    done.setAttribute("data-modal-primary", "");
    done.addEventListener("click", () => void renderList());
    actions.appendChild(done);
    modal.appendChild(actions);
  };

  await renderList();
}
