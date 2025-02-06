import matplotlib.pyplot as plt
import numpy as np
import csv


# Function to read and plot data from a CSV file
def plot_data(file_name, ax, offset):
    with open(file_name, "r") as file:
        reader = csv.reader(file)
        next(reader)  # Skip the header row
        data = list(reader)

    # Extract the data
    names = [row[0] for row in data]
    time_5k = [float(row[7]) for row in data]
    time_50k = [float(row[6]) for row in data]
    time_200k = [float(row[5]) for row in data]
    cpu_time_5k = [float(row[4]) for row in data]
    cpu_time_50k = [float(row[3]) for row in data]
    cpu_time_200k = [float(row[2]) for row in data]

    # Prepare the data for plotting
    x = np.arange(len(names)) + offset  # the label locations with offset
    width = 0.125  # the width of the bars

    # Plot Time for 5k requests
    rects = ax.bar(x, time_5k, width, label="Time 5k")
    ax.bar_label(rects, padding=3, fontsize=7, rotation=90)

    # Plot Time for 50k requests
    rects = ax.bar(x + width, time_50k, width, label="Time 50k")
    ax.bar_label(rects, padding=3, fontsize=7, rotation=90)

    # Plot Time for 200k requests
    rects = ax.bar(x + 2 * width, time_200k, width, label="Time 200k")
    ax.bar_label(rects, padding=3, fontsize=7, rotation=90)

    # Plot CPU time for 5k requests
    rects = ax.bar(x + 3 * width, cpu_time_5k, width, label="CPU Time 5k")
    ax.bar_label(rects, padding=3, fontsize=7, rotation=90)

    # Plot CPU time for 50k requests
    rects = ax.bar(x + 4 * width, cpu_time_50k, width, label="CPU Time 50k")
    ax.bar_label(rects, padding=3, fontsize=7, rotation=90)

    # Plot CPU time for 200k requests
    rects = ax.bar(x + 5 * width, cpu_time_200k, width, label="CPU Time 200k")
    ax.bar_label(rects, padding=3, fontsize=7, rotation=90)

    return x, width, names


# Create a figure with three subplots
fig, (ax1, ax2, ax3) = plt.subplots(3, 1, figsize=(10, 7), layout="constrained")

x1, width, names = plot_data("session=False.csv", ax1, 0)
x2, _, _ = plot_data("session=True.csv", ax2, 0)
x3, _, x3names = plot_data("session='Async'.csv", ax3, 0)


# Adjust the y-axis limits for the first subplot
y_min, y_max = ax1.get_ylim()
new_y_max = y_max + 7
ax1.set_ylim(y_min, new_y_max)

# Adjust the y-axis limits for the second subplot
y_min, y_max = ax2.get_ylim()
new_y_max = y_max + 2
ax2.set_ylim(y_min, new_y_max)

# Adjust the y-axis limits for the third subplot
y_min, y_max = ax3.get_ylim()
new_y_max = y_max + 2
ax3.set_ylim(y_min, new_y_max)

# Add some text for labels, title and custom x-axis tick labels, etc.
ax1.set_ylabel("Time (s)")
ax1.set_title(
    "Benchmark get(url).text | Session=False | Requests: 400 | Response: gzip, utf-8, size 5Kb,50Kb,200Kb"
)
ax1.set_xticks(
    x1 + 3 * width - width / 2
)  # Adjust the x-ticks to be after the 3rd bar, moved 0.5 bar width to the left
ax1.set_xticklabels(names)
ax1.legend(loc="upper left", ncols=6, prop={"size": 8})
ax1.tick_params(axis="x", labelsize=8)

ax2.set_ylabel("Time (s)")
ax2.set_title(
    "Benchmark get(url).text | Session=True | Requests: 400 | Response: gzip, utf-8, size 5Kb,50Kb,200Kb"
)
ax2.set_xticks(
    x2 + 3 * width - width / 2
)  # Adjust the x-ticks to be after the 3rd bar, moved 0.5 bar width to the left
ax2.set_xticklabels(names)
ax2.legend(loc="upper left", ncols=6, prop={"size": 8})
ax2.tick_params(axis="x", labelsize=8)

ax3.set_ylabel("Time (s)")
ax3.set_title(
    "Benchmark get(url).text | Session=Async | Requests: 400 | Response: gzip, utf-8, size 5Kb,50Kb,200Kb"
)
ax3.set_xticks(
    x3 + 3 * width - width / 2
)  # Adjust the x-ticks to be after the 3rd bar, moved 0.5 bar width to the left
ax3.set_xticklabels(x3names)
ax3.legend(loc="upper left", ncols=6, prop={"size": 8})
ax3.tick_params(axis="x", labelsize=8)

# Save the plot to a file
plt.savefig("benchmark.jpg", format="jpg", dpi=80, bbox_inches="tight")
