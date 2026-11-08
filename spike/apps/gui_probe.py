# A1: can a process started by a launchd agent (gui/<uid>) open a window? Writes the result, closes itself.
import sys, tkinter as tk
out = sys.argv[1]
r = tk.Tk(); r.title('Devid A1 probe'); tk.Label(r, text='Devid A1: GUI from launchd', padx=24, pady=16).pack()
def check():
    open(out, 'w').write(f'viewable={r.winfo_viewable()} screen={r.winfo_screenwidth()}x{r.winfo_screenheight()}\n'); r.destroy()
r.after(2000, check); r.mainloop()
