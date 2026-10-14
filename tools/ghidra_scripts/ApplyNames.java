// Applies re/names.csv (address,name,comment) to the current program: renames functions
// (creating them if missing) and sets plate comments. Rerun after editing the CSV.
//@category EatThis
import java.io.*;
import java.nio.file.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.SourceType;

public class ApplyNames extends GhidraScript {
	@Override
	public void run() throws Exception {
		int n = 0;
		for (String line : Files.readAllLines(Paths.get("re/names.csv"))) {
			if (line.isBlank() || line.startsWith("address,")) continue;
			String[] f = line.split(",", 3);
			Address a = currentProgram.getAddressFactory().getAddress(f[0]);
			Function fn = getFunctionAt(a);
			if (fn == null) fn = createFunction(a, f[1]);
			if (fn == null) { printerr("no function at " + f[0]); continue; }
			fn.setName(f[1], SourceType.USER_DEFINED);
			if (f.length > 2 && !f[2].isBlank()) fn.setComment(f[2]);
			n++;
		}
		println("named " + n + " functions");
	}
}
