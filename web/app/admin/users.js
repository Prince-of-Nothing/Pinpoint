// Admin → Users: list, search, role/active changes, safe purge.
// All authorization is enforced by /api/v1/admin/*; this file is only a view.

export const usersModule = {
  id: "users",
  title: "Users",
  order: 20,
  async mount(el, ctx) {
    el.innerHTML = `
      <details class="admin-create">
        <summary>Create user</summary>
        <form id="admin-create-user">
          <label>Email
            <input id="admin-create-email" type="email" autocomplete="off" maxlength="254" required />
          </label>
          <label>Full name
            <input id="admin-create-name" type="text" autocomplete="off" maxlength="100" required />
          </label>
          <label>Role
            <select id="admin-create-role">
              <option value="resident" selected>resident</option>
              <option value="admin">admin</option>
              <option value="administrator">administrator</option>
            </select>
          </label>
          <label>Temporary password
            <input id="admin-create-password" type="password" minlength="8" maxlength="128" autocomplete="new-password" required />
          </label>
          <p class="muted">The user must change this temporary password at first sign-in.</p>
          <button type="submit">Create user</button>
        </form>
      </details>
      <details class="admin-reset">
        <summary>Reset password</summary>
        <form id="admin-reset-user">
          <label>User
            <select id="admin-reset-target" required></select>
          </label>
          <label>Temporary password
            <input id="admin-reset-password" type="password" minlength="8" maxlength="128" autocomplete="new-password" required />
          </label>
          <label class="admin-reset-mfa">
            <input id="admin-reset-mfa" type="checkbox" />
            Clear MFA
          </label>
          <p class="muted">The reset revokes the account's sessions and forces a password change at next sign-in.</p>
          <button type="submit">Reset password</button>
        </form>
      </details>
      <div class="admin-toolbar">
        <input id="admin-user-search" type="search" placeholder="Filter by email or name…" aria-label="Filter users" />
        <button type="button" id="admin-users-refresh" class="secondary">Refresh</button>
      </div>
      <div id="admin-users-table"></div>`;
    const createForm = el.querySelector("#admin-create-user");
    const resetForm = el.querySelector("#admin-reset-user");
    const resetTarget = el.querySelector("#admin-reset-target");
    const resetPassword = el.querySelector("#admin-reset-password");
    const resetMfa = el.querySelector("#admin-reset-mfa");
    const search = el.querySelector("#admin-user-search");
    const table = el.querySelector("#admin-users-table");
    let users = [];

    async function load() {
      const data = await ctx.api("/api/v1/admin/users");
      users = Array.isArray(data?.users) ? data.users : [];
      render();
    }

    function render() {
      const me = ctx.getUser?.()?.id;
      const previousTarget = resetTarget.value;
      const targets = users.filter((u) => u.id !== me);
      resetTarget.innerHTML = targets.length
        ? targets
            .map(
              (u) =>
                `<option value="${u.id}">${escapeHtml(u.full_name)} (${escapeHtml(u.email)})</option>`,
            )
            .join("")
        : '<option value="">No other users</option>';
      if (targets.some((u) => String(u.id) === previousTarget)) {
        resetTarget.value = previousTarget;
      }
      const needle = search.value.trim().toLowerCase();
      const rows = users.filter((u) => {
        if (!needle) return true;
        return `${u.email} ${u.full_name} ${u.role}`.toLowerCase().includes(needle);
      });
      if (!rows.length) {
        table.innerHTML = '<p class="muted">No matching users.</p>';
        return;
      }
      table.innerHTML = `
        <table class="admin-table">
          <thead>
            <tr><th>User</th><th>Role</th><th>Active</th><th></th></tr>
          </thead>
          <tbody>
            ${rows
              .map(
                (u) => `
              <tr data-id="${u.id}">
                <td>
                  <strong>${escapeHtml(u.full_name)}</strong>
                  <span class="muted">${escapeHtml(u.email)}</span>
                  <span class="badge ${u.auth_method === "anonymous" ? "pending" : "accepted"}">${escapeHtml(u.auth_method)}</span>
                  ${u.must_change_password ? '<span class="badge pending">must change password</span>' : ""}
                  ${u.mfa_enabled ? '<span class="badge accepted">MFA</span>' : ""}
                </td>
                <td>
                  <select data-action="role" ${u.id === me ? "disabled" : ""}>
                    ${["resident", "admin", "administrator"]
                      .map((r) => `<option value="${r}" ${r === u.role ? "selected" : ""}>${r}</option>`)
                      .join("")}
                  </select>
                </td>
                <td>
                  <input type="checkbox" data-action="active" ${u.is_active ? "checked" : ""} ${u.id === me ? "disabled" : ""} aria-label="Active" />
                </td>
                <td class="admin-actions">
                  <button type="button" class="secondary" data-action="reset" ${u.id === me ? "disabled" : ""}>Reset password</button>
                  <button type="button" class="secondary" data-action="delete" ${u.id === me ? "disabled" : ""}>Delete</button>
                </td>
              </tr>`,
              )
              .join("")}
          </tbody>
        </table>`;
    }

    table.addEventListener("change", async (event) => {
      const target = event.target;
      const row = target.closest("tr[data-id]");
      if (!row) return;
      const id = Number(row.dataset.id);
      try {
        if (target.dataset.action === "role") {
          await ctx.api(`/api/v1/admin/users/${id}`, { method: "PATCH", body: { role: target.value } });
          ctx.setStatus("Role updated.");
          await load();
        }
        if (target.dataset.action === "active") {
          await ctx.api(`/api/v1/admin/users/${id}`, {
            method: "PATCH",
            body: { is_active: target.checked },
          });
          ctx.setStatus("User updated.");
          await load();
        }
      } catch (err) {
        ctx.setStatus(err.message, true);
        await load();
      }
    });

    table.addEventListener("click", async (event) => {
      const target = event.target;
      const row = target.closest("tr[data-id]");
      if (!row) return;
      const id = Number(row.dataset.id);
      const user = users.find((u) => u.id === id);
      if (!user) return;
      if (target.dataset.action === "reset") {
        const details = el.querySelector(".admin-reset");
        details.open = true;
        resetTarget.value = String(id);
        resetPassword.focus();
        return;
      }
      if (target.dataset.action !== "delete") return;
      const ok = window.confirm(
        `Delete ${user.email}?\n\nThis removes the account, sessions, friendships, shares, locations, check-ins and emergencies. This cannot be undone.`,
      );
      if (!ok) return;
      try {
        await ctx.api(`/api/v1/admin/users/${id}`, { method: "DELETE" });
        ctx.setStatus("User deleted.");
        await load();
      } catch (err) {
        ctx.setStatus(err.message, true);
      }
    });

    search.addEventListener("input", render);
    el.querySelector("#admin-users-refresh").addEventListener("click", load);
    createForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const email = el.querySelector("#admin-create-email").value.trim();
      const fullName = el.querySelector("#admin-create-name").value.trim();
      const role = el.querySelector("#admin-create-role").value;
      const password = el.querySelector("#admin-create-password").value;
      try {
        await ctx.api("/api/v1/users/", {
          method: "POST",
          body: { email, full_name: fullName, role, password },
        });
        ctx.setStatus("User created. Share the temporary password securely.");
        createForm.reset();
        await load();
      } catch (err) {
        ctx.setStatus(err.message, true);
      }
    });
    resetForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const id = Number(resetTarget.value);
      const password = resetPassword.value;
      if (!id) {
        ctx.setStatus("Choose a user to reset.", true);
        return;
      }
      try {
        await ctx.api(`/api/v1/admin/users/${id}/password`, {
          method: "POST",
          body: { password, reset_mfa: resetMfa.checked },
        });
        ctx.setStatus("Temporary password issued. Share it securely.");
        resetForm.reset();
        await load();
      } catch (err) {
        ctx.setStatus(err.message, true);
      }
    });
    await load();
  },
};

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, (c) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  }[c]));
}
