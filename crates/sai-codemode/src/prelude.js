(function (emit, seedJson) {
    "use strict";
    const seed = JSON.parse(seedJson);
    const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
    const stringify = JSON.stringify;
    const parse = JSON.parse;
    const pending = new Map();
    const values = Object.assign(Object.create(null), seed.store);
    const writes = Object.create(null);
    const exitSignal = Symbol("exit");
    let sequence = 0;
    let finished = false;
    let outputBytes = 0;
    let outputItems = 0;

    // 1. 【CodeMode】【边界封装】桥接函数仅在闭包中保存，不暴露原生发送能力
    /** 【CodeMode】【值格式化】参数 value 为脚本值，返回可输出文本。 */
    function format(value) {
        return typeof value === "string" ? value : (stringify(value) ?? "undefined");
    }
    /** 【CodeMode】【输出发送】参数 kind/value 为类型和值，校验容量后交付输出。 */
    function output(kind, value) {
        const formatted = format(value);
        outputBytes += formatted.length;
        if (outputBytes > 16777216 || ++outputItems > 100000) {
            throw new Error("codemode output exceeds size limit");
        }
        emit(stringify({ type: "output", kind, value: formatted }));
    }
    /** 【CodeMode】【异步调用】参数 name/argumentsValue 为工具和参数，返回宿主结果 Promise。 */
    function call(name, argumentsValue = {}) {
        if (finished) return Promise.reject(new Error("script already completed"));
        if (++sequence > 1024 || pending.size >= 128) {
            return Promise.reject(new Error("codemode tool call limit exceeded"));
        }
        const id = sequence;
        return new Promise((resolve, reject) => {
            pending.set(id, { resolve, reject });
            try {
                emit(stringify({ type: "call", id, name, arguments: argumentsValue }));
            } catch (error) {
                pending.delete(id);
                reject(error);
            }
        });
    }
    /** 【CodeMode】【深度冻结】参数 value/seen 为对象和已访问集合，返回保护后的对象。 */
    function freeze(value, seen = new Set()) {
        if (value === null || (typeof value !== "object" && typeof value !== "function") || seen.has(value)) return value;
        seen.add(value);
        for (const descriptor of Object.values(Object.getOwnPropertyDescriptors(value))) {
            if ("value" in descriptor) freeze(descriptor.value, seen);
            if (descriptor.get) freeze(descriptor.get, seen);
            if (descriptor.set) freeze(descriptor.set, seen);
        }
        return Object.freeze(value);
    }
    /** 【CodeMode】【全局声明】参数 name/value 为名称和值，返回无，禁止脚本替换桥接能力。 */
    function expose(name, value) {
        Object.defineProperty(globalThis, name, { value: freeze(value), writable: false, configurable: false });
    }

    const tools = Object.create(null);
    for (const entry of seed.catalog) {
        tools[entry.name] = (args) => call(entry.name, args);
    }
    // 2. 【CodeMode】【别名生成】冲突时保留原名称访问，防止工具间覆盖
    for (const entry of seed.catalog) {
        let alias = entry.name.replace(/[^a-zA-Z0-9_$]/g, "_");
        if (/^[0-9]/.test(alias)) alias = "_" + alias;
        if (!(alias in tools)) tools[alias] = tools[entry.name];
    }
    expose("tools", tools);
    expose("ALL_TOOLS", seed.catalog.map(({ name, description }) => ({ name, description })));
    expose("text", (value) => output("text", value));
    expose("image", (value) => {
        let url = typeof value === "string" ? value : value?.image_url;
        if (!url && value?.type === "image") url = `data:${value.mimeType};base64,${value.data}`;
        if (typeof url !== "string" || !/^data:image\/(png|jpeg|gif|webp);base64,[A-Za-z0-9+/=\r\n]+$/.test(url)) {
            throw new Error("image() requires a base64 PNG, JPEG, GIF or WebP data URL");
        }
        output("image", url);
    });
    expose("console", Object.fromEntries(["log", "info", "error", "debug"].map((name) => [name, (...args) => output("console", args.map(format).join(" "))])));
    expose("exit", () => { throw exitSignal; });
    expose("load", (key) => {
        if (typeof key !== "string") throw new Error("store key must be a string");
        return key in values ? parse(stringify(values[key])) : undefined;
    });
    expose("store", (key, value) => {
        if (typeof key !== "string" || key.length > 1024) throw new Error("invalid store key");
        const before = values[key];
        const existed = key in values;
        const json = value === undefined ? undefined : stringify(value);
        if (value !== undefined && json === undefined) throw new Error("store requires a JSON value");
        if (json && json.length > 262144) throw new Error("codemode store value exceeds size limit");
        if (json === undefined) delete values[key];
        else values[key] = parse(json);
        if (stringify(values).length > 1048576) {
            if (existed) values[key] = before;
            else delete values[key];
            throw new Error("codemode store exceeds total size limit");
        }
        writes[key] = json === undefined ? null : { value: values[key] };
    });
    expose("describeTool", async (name) => seed.catalog.find((entry) => entry.name === name));
    expose("searchTools", async (query, options = {}) => {
        const terms = String(query).toLowerCase().split(/\s+/).filter(Boolean);
        const limit = Math.max(0, Math.min(50, Number(options.limit ?? 8) || 0));
        return seed.catalog.filter((entry) => !options.namespace || entry.name.startsWith(options.namespace))
            .map((entry) => ({ entry, score: terms.reduce((score, term) => score + (entry.name.toLowerCase().includes(term) ? 3 : 0) + (entry.description.toLowerCase().includes(term) ? 1 : 0), 0) }))
            .filter(({ score }) => score > 0 || terms.length === 0)
            .sort((a, b) => b.score - a.score || a.entry.name.localeCompare(b.entry.name))
            .slice(0, limit).map(({ entry: { name, description } }) => ({ name, description }));
    });

    // 3. 【CodeMode】【内置保护】冻结可到达的内置原型，仍允许脚本创建普通实例
    for (const name of Object.getOwnPropertyNames(globalThis)) {
        const descriptor = Object.getOwnPropertyDescriptor(globalThis, name);
        if (name !== "globalThis" && descriptor && "value" in descriptor) {
            freeze(descriptor.value);
            if (descriptor.configurable) Object.defineProperty(globalThis, name, { ...descriptor, writable: false, configurable: false });
        }
    }
    freeze(AsyncFunction);

    return {
        async run(code) {
            try {
                const result = await new AsyncFunction('"use strict";\n' + code)();
                if (result !== undefined) output("text", result);
                finished = true;
                return stringify({ error: null, writes });
            } catch (error) {
                finished = true;
                if (error === exitSignal) return stringify({ error: null, writes });
                return stringify({ error: String(error) + (error?.stack ? "\n" + error.stack : ""), writes: {} });
            }
        },
        resolve(json) {
            const reply = parse(json);
            const slot = pending.get(reply.id);
            if (!slot) throw new Error("unexpected host reply");
            pending.delete(reply.id);
            if (reply.error !== null) slot.reject(new Error(reply.error));
            else slot.resolve(reply.value);
        },
        pendingCount() { return pending.size; }
    };
})
