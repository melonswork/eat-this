// Lists every imported (external) function and the program functions that call it,
// writing work/decomp/<program>.imports.txt. This is the map of the platform layer.
//@category EatThis
import java.io.*;
import java.util.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;

public class ImportMap extends GhidraScript {
	@Override
	public void run() throws Exception {
		File out = new File("work/decomp/" + currentProgram.getName() + ".imports.txt");
		out.getParentFile().mkdirs();
		FunctionManager fm = currentProgram.getFunctionManager();
		ReferenceManager rm = currentProgram.getReferenceManager();
		try (PrintWriter w = new PrintWriter(new FileWriter(out))) {
			for (Symbol s : currentProgram.getSymbolTable().getExternalSymbols()) {
				TreeSet<String> callers = new TreeSet<>();
				for (Reference r : rm.getReferencesTo(s.getAddress())) {
					Function f = fm.getFunctionContaining(r.getFromAddress());
					callers.add(f == null ? r.getFromAddress().toString() : f.getName());
				}
				// NE imports are reached through thunks; follow them one level.
				Function ext = fm.getFunctionAt(s.getAddress());
				if (ext != null) {
					for (Function thunk : ext.getFunctionThunkAddresses(true) == null ? new Function[0] : Arrays.stream(ext.getFunctionThunkAddresses(true)).map(fm::getFunctionAt).toArray(Function[]::new)) {
						for (Reference r : rm.getReferencesTo(thunk.getEntryPoint())) {
							Function f = fm.getFunctionContaining(r.getFromAddress());
							if (f != null) callers.add(f.getName());
						}
					}
				}
				w.println(s.getParentNamespace().getName() + "::" + s.getName() + "\t" + String.join(" ", callers));
			}
		}
		println("wrote " + out);
	}
}
