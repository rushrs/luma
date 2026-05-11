local mode_file = vim.fn.expand("~/.cache/luma/mode")
local theme_file = vim.fn.expand("~/.cache/luma/nvim-colorscheme")
local mode_dir = vim.fn.fnamemodify(mode_file, ":h")
local mode_name = vim.fn.fnamemodify(mode_file, ":t")
local theme_name = vim.fn.fnamemodify(theme_file, ":t")
local last_mode
local last_theme

local function read_first(path)
  local ok, lines = pcall(vim.fn.readfile, path)
  if ok and lines and lines[1] and lines[1] ~= "" then return vim.trim(lines[1]) end
end

local function read_theme_mode()
  local mode = read_first(mode_file)
  if mode == "dark" or mode == "light" then return mode end

  local result = vim.fn.system({ "defaults", "read", "-g", "AppleInterfaceStyle" })
  if vim.v.shell_error == 0 and result:match("Dark") then return "dark" end

  return "light"
end

local function read_theme_name(mode)
  return read_first(theme_file) or (mode == "dark" and "carbonfox" or "dawnfox")
end

local function apply_luma(force)
  local mode = read_theme_mode()
  local target = read_theme_name(mode)
  if not force and mode == last_mode and target == last_theme then return end
  last_mode = mode
  last_theme = target

  vim.o.background = mode
  if vim.g.colors_name ~= target then pcall(vim.cmd.colorscheme, target) end
end

vim.api.nvim_create_user_command("LumaSync", function() apply_luma(true) end, { desc = "Sync theme with OS appearance via Luma" })

local group = vim.api.nvim_create_augroup("LumaSync", { clear = true })
vim.api.nvim_create_autocmd({ "VimEnter", "FocusGained", "TermEnter", "BufEnter", "WinEnter" }, {
  group = group,
  callback = function() apply_luma(false) end,
})

local fs_event = vim.uv.new_fs_event()
if fs_event then
  fs_event:start(
    mode_dir,
    {},
    vim.schedule_wrap(function(_, filename)
      if filename == nil or filename == mode_name or filename == theme_name then pcall(apply_luma, true) end
    end)
  )
  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = group,
    callback = function()
      if not fs_event:is_closing() then fs_event:close() end
    end,
  })
end

local timer = vim.uv.new_timer()
if timer then
  timer:start(
    2000,
    2000,
    vim.schedule_wrap(function()
      pcall(apply_luma, false)
    end)
  )
  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = group,
    callback = function()
      if not timer:is_closing() then timer:close() end
    end,
  })
end
