local version = nodejs and nodejs.version
local prefix = version and green("node:" .. version .. " ") or ""

return { format = prefix .. (ctx.pwd or "") .. " ❯ " }
