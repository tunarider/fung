import subprocess
print(sum(subprocess.run(["true"], capture_output=True).returncode for _ in range(500)))
