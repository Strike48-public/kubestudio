/// YAML templates for common Kubernetes resources
pub struct ResourceTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub yaml: &'static str,
}

pub const TEMPLATES: &[ResourceTemplate] = &[
    ResourceTemplate {
        name: "Pod",
        description: "Basic pod with single container",
        yaml: r#"apiVersion: v1
kind: Pod
metadata:
  name: my-pod
  namespace: default
  labels:
    app: my-app
spec:
  containers:
  - name: main
    image: nginx:latest
    ports:
    - containerPort: 80
    resources:
      requests:
        memory: "64Mi"
        cpu: "100m"
      limits:
        memory: "128Mi"
        cpu: "200m"
"#,
    },
    ResourceTemplate {
        name: "Deployment",
        description: "Deployment with replicas and rolling update",
        yaml: r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: my-deployment
  namespace: default
  labels:
    app: my-app
spec:
  replicas: 3
  selector:
    matchLabels:
      app: my-app
  template:
    metadata:
      labels:
        app: my-app
    spec:
      containers:
      - name: main
        image: nginx:latest
        ports:
        - containerPort: 80
        resources:
          requests:
            memory: "64Mi"
            cpu: "100m"
          limits:
            memory: "128Mi"
            cpu: "200m"
"#,
    },
    ResourceTemplate {
        name: "Service (ClusterIP)",
        description: "Internal cluster service",
        yaml: r#"apiVersion: v1
kind: Service
metadata:
  name: my-service
  namespace: default
spec:
  type: ClusterIP
  selector:
    app: my-app
  ports:
  - port: 80
    targetPort: 80
    protocol: TCP
"#,
    },
    ResourceTemplate {
        name: "Service (NodePort)",
        description: "Service exposed on node ports",
        yaml: r#"apiVersion: v1
kind: Service
metadata:
  name: my-nodeport-service
  namespace: default
spec:
  type: NodePort
  selector:
    app: my-app
  ports:
  - port: 80
    targetPort: 80
    nodePort: 30080
    protocol: TCP
"#,
    },
    ResourceTemplate {
        name: "Service (LoadBalancer)",
        description: "Service with external load balancer",
        yaml: r#"apiVersion: v1
kind: Service
metadata:
  name: my-lb-service
  namespace: default
spec:
  type: LoadBalancer
  selector:
    app: my-app
  ports:
  - port: 80
    targetPort: 80
    protocol: TCP
"#,
    },
    ResourceTemplate {
        name: "ConfigMap",
        description: "Configuration data storage",
        yaml: r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: my-config
  namespace: default
data:
  config.yaml: |
    key: value
    nested:
      setting: enabled
  APP_ENV: production
"#,
    },
    ResourceTemplate {
        name: "Secret",
        description: "Opaque secret (base64 encoded)",
        yaml: r#"apiVersion: v1
kind: Secret
metadata:
  name: my-secret
  namespace: default
type: Opaque
stringData:
  username: admin
  password: changeme
"#,
    },
    ResourceTemplate {
        name: "Job",
        description: "One-time batch job",
        yaml: r#"apiVersion: batch/v1
kind: Job
metadata:
  name: my-job
  namespace: default
spec:
  ttlSecondsAfterFinished: 100
  template:
    spec:
      containers:
      - name: job
        image: busybox
        command: ["echo", "Hello from job"]
      restartPolicy: Never
  backoffLimit: 4
"#,
    },
    ResourceTemplate {
        name: "CronJob",
        description: "Scheduled recurring job",
        yaml: r#"apiVersion: batch/v1
kind: CronJob
metadata:
  name: my-cronjob
  namespace: default
spec:
  schedule: "*/5 * * * *"
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: job
            image: busybox
            command: ["echo", "Hello from cronjob"]
          restartPolicy: OnFailure
"#,
    },
    ResourceTemplate {
        name: "Ingress",
        description: "HTTP/HTTPS ingress rule",
        yaml: r#"apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: my-ingress
  namespace: default
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
spec:
  rules:
  - host: myapp.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: my-service
            port:
              number: 80
"#,
    },
    ResourceTemplate {
        name: "PersistentVolumeClaim",
        description: "Storage volume claim",
        yaml: r#"apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: my-pvc
  namespace: default
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 1Gi
  storageClassName: standard
"#,
    },
    ResourceTemplate {
        name: "StatefulSet",
        description: "Stateful application with stable network identity",
        yaml: r#"apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: my-statefulset
  namespace: default
spec:
  serviceName: my-statefulset
  replicas: 3
  selector:
    matchLabels:
      app: my-statefulset
  template:
    metadata:
      labels:
        app: my-statefulset
    spec:
      containers:
      - name: main
        image: nginx:latest
        ports:
        - containerPort: 80
        volumeMounts:
        - name: data
          mountPath: /data
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 1Gi
"#,
    },
    ResourceTemplate {
        name: "DaemonSet",
        description: "Pod on every node",
        yaml: r#"apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: my-daemonset
  namespace: default
spec:
  selector:
    matchLabels:
      app: my-daemonset
  template:
    metadata:
      labels:
        app: my-daemonset
    spec:
      containers:
      - name: main
        image: nginx:latest
        resources:
          limits:
            memory: "128Mi"
            cpu: "100m"
"#,
    },
];
