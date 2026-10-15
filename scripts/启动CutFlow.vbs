Option Explicit
Dim shell, files, folder, state, environment
Set shell = CreateObject("WScript.Shell")
Set files = CreateObject("Scripting.FileSystemObject")
folder = files.GetParentFolderName(WScript.ScriptFullName)
state = files.BuildPath(folder, "portable")
If Not files.FolderExists(state) Then files.CreateFolder state
If Not files.FolderExists(files.BuildPath(state, "appdata")) Then files.CreateFolder files.BuildPath(state, "appdata")
If Not files.FolderExists(files.BuildPath(state, "localappdata")) Then files.CreateFolder files.BuildPath(state, "localappdata")
If Not files.FolderExists(files.BuildPath(state, "temp")) Then files.CreateFolder files.BuildPath(state, "temp")
Set environment = shell.Environment("PROCESS")
environment("APPDATA") = files.BuildPath(state, "appdata")
environment("LOCALAPPDATA") = files.BuildPath(state, "localappdata")
environment("TEMP") = files.BuildPath(state, "temp")
environment("TMP") = files.BuildPath(state, "temp")
shell.CurrentDirectory = folder
shell.Run Chr(34) & files.BuildPath(folder, "CutFlow.exe") & Chr(34), 1, False
