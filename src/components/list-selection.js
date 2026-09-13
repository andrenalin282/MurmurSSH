/** Pure selection math shared by the local and remote file browsers. */
export function clickSelect(names, state, name, mods) {
    const selected = new Set(state.selected);
    if (mods.ctrl) {
        if (selected.has(name))
            selected.delete(name);
        else
            selected.add(name);
        return { selected, anchor: name, cursor: name };
    }
    if (mods.shift && state.anchor) {
        const a = names.indexOf(state.anchor);
        const b = names.indexOf(name);
        if (a >= 0 && b >= 0) {
            for (let i = Math.min(a, b); i <= Math.max(a, b); i++)
                selected.add(names[i]);
            return { selected, anchor: state.anchor, cursor: name };
        }
    }
    return { selected: new Set([name]), anchor: name, cursor: name };
}
export function moveCursor(names, state, delta, extend) {
    if (names.length === 0)
        return state;
    const cur = state.cursor ? names.indexOf(state.cursor) : -1;
    let next;
    if (delta === "start")
        next = 0;
    else if (delta === "end")
        next = names.length - 1;
    else if (cur < 0)
        next = delta > 0 ? 0 : names.length - 1;
    else
        next = Math.max(0, Math.min(names.length - 1, cur + delta));
    const target = names[next];
    if (!extend)
        return { selected: new Set([target]), anchor: target, cursor: target };
    const anchorName = state.anchor ?? state.cursor ?? target;
    const a0 = names.indexOf(anchorName);
    const a = a0 < 0 ? next : a0;
    const anchor = a0 < 0 ? names[a] : anchorName;
    const selected = new Set();
    for (let i = Math.min(a, next); i <= Math.max(a, next); i++)
        selected.add(names[i]);
    return { selected, anchor, cursor: target };
}
