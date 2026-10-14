// Prints a function count and decompiles the program entry, as a smoke test of the analysis.
//@category EatThis
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;

public class Summary extends GhidraScript {
	@Override
	public void run() throws Exception {
		FunctionManager fm = currentProgram.getFunctionManager();
		println("functions: " + fm.getFunctionCount());
		DecompInterface di = new DecompInterface();
		di.openProgram(currentProgram);
		var entries = currentProgram.getSymbolTable().getExternalEntryPointIterator();
		while (entries.hasNext()) {
			Function f = fm.getFunctionAt(entries.next());
			if (f == null) continue;
			println("=== " + f.getName() + " @ " + f.getEntryPoint());
			DecompileResults r = di.decompileFunction(f, 60, monitor);
			println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : r.getErrorMessage());
			break;
		}
	}
}
