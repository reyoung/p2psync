# P2PSync

A peer-to-peer file synchronization tool written in Rust.


## Usage

1. Start the tracker

    Tracker is used to discover peers. find a node that IP is fixed, run 
    
    ```bash
    p2psync tracker -p 9090
    ```
   
2. Start the server that provides files

    NOTE: The directory should be unchanged for now.

   ```bash
   mkdir -p models
   cd models
   ln -s /path/to/your/model/dir2 model1
   ln -s /path/to/your/model/dir2 model2
   cd ..
   ~/projs/p2psync/target/debug/p2psync serve \
    --tracker http://{TRACKER_URL}:9090 --address {LOCAL_IP} \
    --path models \
    --dump-path spec.bin
   ```
   
    When the server is started, it will dump the md5 spec to `spec.bin`. Then the sever can be faster to start by loading the spec.

   ```bash
    ~/projs/p2psync/target/debug/p2psync serve \
    --tracker http://{TRACKER_URL}:9090 --address {LOCAL_IP} \
    --load-path spec.bin
   ```
   

3. Download the files
    
    The server will print the md5 of files and directories. Use these md5 to download the files.

    ```bash
    p2psync download --md5 {FILE/DIR MD5} --tracker http://{TRACKER_IP}:9090
    ```
   
4. Start the server when files are downloaded
5. 
    You can start the server when files are downloaded. So the file sync will be faster than the first time,
    because there are multiple servers that provide the same files.