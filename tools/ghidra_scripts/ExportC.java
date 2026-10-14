// Decompiles every function of the current program into work/decomp/<program>.c.
//@category EatThis
import java.io.*;
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;

public class ExportC extends GhidraScript {
	@Override
	public void run() throws Exception {
		File out = new File("work/decomp/" + currentProgram.getName() + ".c");
		out.getParentFile().mkdirs();
		DecompInterface di = new DecompInterface();
		di.openProgram(currentProgram);
		try (PrintWriter w = new PrintWriter(new FileWriter(out))) {
			for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
				DecompileResults r = di.decompileFunction(f, 120, monitor);
				w.println("// ===== " + f.getName() + " @ " + f.getEntryPoint());
				w.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// FAILED: " + r.getErrorMessage());
			}
		}
		println("wrote " + out);
	}
}
