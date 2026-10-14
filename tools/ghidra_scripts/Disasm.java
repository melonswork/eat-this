// Writes the disassembly of the functions named in the script arguments to work/decomp/<name>.asm.
//@category EatThis
import java.io.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;

public class Disasm extends GhidraScript {
	@Override
	public void run() throws Exception {
		for (String name : getScriptArgs()) {
			if (name.isBlank()) continue;
			Function f = getGlobalFunctions(name.trim()).stream().findFirst().orElse(null);
			if (f == null) { printerr("no function " + name); continue; }
			try (PrintWriter w = new PrintWriter(new FileWriter("work/decomp/" + name.trim() + ".asm"))) {
				for (Instruction i : currentProgram.getListing().getInstructions(f.getBody(), true)) {
					w.println(i.getAddress() + "  " + i);
				}
			}
		}
	}
}
