import os
import subprocess


def decode(s, encodings=('ascii', 'utf8', 'latin1')):
    for encoding in encodings:
        try:
            return s.decode(encoding)
        except UnicodeDecodeError:
            pass
    return s.decode('ascii', 'ignore')


def run_bfjit(f, opts):
    proc = subprocess.Popen(["cargo", "run", "--release", "--", *opts, f"../sample_programs/{f}"],
                            stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE)
    out, err = proc.communicate()
    if proc.returncode != 0:
        print(f"process crashed: {err}")

    last_line = decode(out).split("\n")[-2].strip()
    return last_line.split(" ")[0].split("=")[1]


print("| file | interpreted time | jit unoptimized | jit optimized |")
print("|------|------------------|-----------------|---------------|")
for file in os.listdir("../sample_programs"):
    interpreted_time = run_bfjit(file, ["-i"])
    jit_time = run_bfjit(file, ["-j"])
    jit_unoptimized_time = run_bfjit(file, ["-j", "-u"])

    print(f"{file}|{interpreted_time}|{jit_unoptimized_time}|{jit_time}")
