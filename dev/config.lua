local version = nodejs and nodejs.version
local pwd = ctx.pwd or ""

if version then
    return { format = green("node:" .. version) .. " " .. pwd .. " ❯ " }
end

return { format = pwd .. " ❯ " }
