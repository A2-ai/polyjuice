#!/bin/bash

# Check if the script is running as root
if [[ $EUID -ne 0 ]]; then
    echo "This script must be run as root."
    exit 1
fi

# Define the custom home directory root location
HOME_ROOT="/cluster-data/user-homes"

# Define the starting UID
START_UID=100200

# Loop to create 10 users
for ((i=200; i<202; i++)); do
    # Calculate the UID for the current user
    USER_UID=$((START_UID + i))

    # Measure the time it takes to create the user
    start_time=$(date +%s.%N)
    # Create the user with the specified home directory root location and set the login shell to /bin/bash
    useradd -M -s /bin/bash -d "$HOME_ROOT/user$i" -u $USER_UID user$i

    end_time=$(date +%s.%N)
    elapsed_time=$(echo "$end_time - $start_time" | bc)
    
    # Display information about the created user and the time it took
    echo "User user$i created with UID $USER_UID, home directory set to $HOME_ROOT/user$i, and login shell /bin/bash"
    echo "Time taken to create user$i: $elapsed_time seconds"
done

# Display a message when the script is done
echo "User creation completed."
