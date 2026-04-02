local parts = {}

local version = nodejs and nodejs.version
if version then
    table.insert(parts, green("node:" .. version))
end

table.insert(parts, ctx.pwd or "")
table.insert(parts, "❯ ")

return { format = table.concat(parts, " ") }
